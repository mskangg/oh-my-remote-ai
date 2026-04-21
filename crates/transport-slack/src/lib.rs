//! Slack transport layer for Remote Claude Code.
//!
//! Provides [`serve_socket_mode`] (the main WebSocket listener), [`SlackTransport`]
//! (thread→session binding and message routing), and [`SlackWebApiPublisher`]
//! (Slack API calls for posting/updating/deleting messages).  This crate owns
//! Slack payload parsing but does **not** own product business logic.

mod formatting;
mod ports;
mod publisher;
mod socket_mode;
mod store;
mod transport;
mod types;

pub use formatting::{
    build_channel_message_request, build_status_delete_request, build_status_update_request,
    build_thread_message_request, build_thread_message_request_with_blocks,
    claude_md_to_slack_mrkdwn, parse_push_thread_reply,
};
pub use ports::{
    SessionBindingRegistrar, SessionBindingStore, SessionHandleResolver, SessionStatusRegistrar,
    SessionStatusStore, SlackProjectLocator, SlackSessionCatalogStore, SlackSessionOrchestrator,
    SlackSessionPublisher, SlackSessionStarter, SlackStatusMessagePublisher, SlackThreadRouter,
    SlackWorkingStatusPublisher,
};
pub use publisher::SlackWebApiPublisher;
pub use socket_mode::{
    is_allowed_user, parse_allowed_user_ids, serve_socket_mode, SlackSlashCommandPayload,
    SlackSocketModeConfig,
};
pub use store::InMemorySlackBindingStore;
pub use transport::SlackTransport;
pub use types::{
    SlackListedSession, SlackMessageTarget, SlackPostedMessage, SlackProject, SlackSessionStart,
    SlackThreadAction, SlackThreadReply, SlackThreadStatus, StartedSlackSession,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use chrono::Utc;
    use core_model::{SessionId, SessionState, TransportBinding, TransportStatusMessage};
    use core_service::{SessionHandle, SessionRequest, SessionRuntimeConfigurator};
    use serde_json::{json, Value};
    use slack_morphism::prelude::*;
    use tokio::sync::Mutex;

    use super::*;
    use crate::formatting::parse_thread_reply;
    use crate::socket_mode::{
        build_socket_mode_ack, command_requests_new_session, handle_socket_mode_text,
        parse_socket_mode_request, SocketModeRequest,
    };
    use crate::types::SlackEnvelope;

    fn test_slack_config(allowed_user_ids: &[String]) -> SlackSocketModeConfig {
        SlackSocketModeConfig {
            bot_token: "xoxb-test".to_string(),
            app_token: "xapp-test".to_string(),
            allowed_user_ids: allowed_user_ids.to_vec(),
            hook_settings_path: String::new(),
            claude_launch_command: "claude --dangerously-skip-permissions".to_string(),
        }
    }

    #[derive(Clone, Default)]
    struct RecordingResolver {
        calls: Arc<Mutex<Vec<SessionId>>>,
        handle: Option<SessionHandle>,
    }

    #[derive(Clone, Default)]
    struct RecordingConfigurator {
        calls: Arc<Mutex<Vec<(SessionId, String)>>>,
    }

    #[async_trait]
    impl SessionRuntimeConfigurator for RecordingConfigurator {
        async fn register_project_root(&self, session_id: SessionId, project_root: &str) -> Result<()> {
            self.calls
                .lock()
                .await
                .push((session_id, project_root.to_string()));
            Ok(())
        }
    }

    #[async_trait]
    impl core_service::SessionRuntimeLiveness for RecordingConfigurator {
        async fn is_session_alive(&self, _session_id: SessionId) -> Result<bool> {
            Ok(true)
        }
    }

    #[derive(Clone, Default)]
    struct RecordingOrchestrator {
        started_channels: Arc<Mutex<Vec<String>>>,
        replies: Arc<Mutex<Vec<SlackThreadReply>>>,
        listed_channels: Arc<Mutex<Vec<String>>>,
        posted_lists: Arc<Mutex<Vec<(String, String)>>>,
        actions: Arc<Mutex<Vec<(String, String, SlackThreadAction)>>>,
    }

    #[async_trait]
    impl SessionHandleResolver for RecordingResolver {
        async fn resolve(&self, session_id: SessionId) -> Result<SessionHandle> {
            self.calls.lock().await.push(session_id);
            self.handle
                .clone()
                .ok_or_else(|| anyhow::anyhow!("missing session handle"))
        }
    }

    #[async_trait]
    impl SlackSessionOrchestrator for RecordingOrchestrator {
        async fn start_new_session(&self, channel_id: &str, _launch_command: String) -> Result<StartedSlackSession> {
            self.started_channels
                .lock()
                .await
                .push(channel_id.to_string());
            Ok(StartedSlackSession {
                session_id: SessionId::new(),
                state: SessionState::Starting,
                binding: TransportBinding {
                    project_space_id: channel_id.to_string(),
                    session_space_id: "1740.100".to_string(),
                },
            })
        }

        async fn handle_session_reply(&self, reply: SlackThreadReply) -> Result<SessionState> {
            self.replies.lock().await.push(reply);
            Ok(SessionState::Idle)
        }

        async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>> {
            self.listed_channels
                .lock()
                .await
                .push(channel_id.to_string());
            Ok(vec![SlackListedSession {
                session_id: SessionId::new(),
                tmux_session_name: "session-1".to_string(),
                thread_ts: "1740.100".to_string(),
                project_label: "demo".to_string(),
                state: SessionState::Idle,
            }])
        }

        async fn post_session_list(&self, channel_id: &str, thread_ts: &str) -> Result<()> {
            self.posted_lists
                .lock()
                .await
                .push((channel_id.to_string(), thread_ts.to_string()));
            Ok(())
        }

        async fn handle_thread_action(
            &self,
            channel_id: &str,
            thread_ts: &str,
            action: SlackThreadAction,
        ) -> Result<Option<SessionState>> {
            self.actions.lock().await.push((
                channel_id.to_string(),
                thread_ts.to_string(),
                action,
            ));
            Ok(Some(SessionState::Idle))
        }
    }

    fn fake_handle(session_id: SessionId, state: SessionState) -> SessionHandle {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<SessionRequest>(1);
        tokio::spawn(async move {
            while let Some(request) = receiver.recv().await {
                let _ = request.reply_tx.send(Ok(state.clone()));
            }
        });

        SessionHandle::new_for_tests(session_id, sender)
    }

    #[derive(Clone, Default)]
    struct RecordingWorkingStatusPublisher {
        statuses: Arc<Mutex<Vec<(SlackMessageTarget, String)>>>,
    }

    #[async_trait]
    impl SlackWorkingStatusPublisher for RecordingWorkingStatusPublisher {
        async fn post_working_status(
            &self,
            target: &SlackMessageTarget,
            text: impl Into<String> + Send,
        ) -> Result<SlackThreadStatus> {
            let text = text.into();
            self.statuses
                .lock()
                .await
                .push((target.clone(), text.clone()));

            Ok(SlackThreadStatus {
                channel_id: target.channel_id.clone(),
                thread_ts: target.thread_ts.clone(),
                status_message_ts: "1740.200".to_string(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingSessionPublisher {
        channel_messages: Arc<Mutex<Vec<(String, String)>>>,
        threaded_block_messages: Arc<Mutex<Vec<(SlackMessageTarget, String, usize)>>>,
        permalink_requests: Arc<Mutex<Vec<(String, String)>>>,
        status_updates: Arc<Mutex<Vec<(SlackThreadStatus, String)>>>,
        fail_updates: Arc<Mutex<bool>>,
        final_replies: Arc<Mutex<Vec<(SlackMessageTarget, String)>>>,
    }

    #[async_trait]
    impl SlackSessionPublisher for RecordingSessionPublisher {
        async fn post_channel_message(&self, channel_id: &str, text: &str) -> Result<SlackPostedMessage> {
            self.channel_messages
                .lock()
                .await
                .push((channel_id.to_string(), text.to_string()));

            Ok(SlackPostedMessage {
                channel_id: channel_id.to_string(),
                message_ts: "1740.100".to_string(),
            })
        }

        async fn post_thread_message_with_blocks(
            &self,
            target: &SlackMessageTarget,
            text: &str,
            blocks: Vec<SlackBlock>,
        ) -> Result<SlackPostedMessage> {
            self.threaded_block_messages.lock().await.push((
                target.clone(),
                text.to_string(),
                blocks.len(),
            ));

            Ok(SlackPostedMessage {
                channel_id: target.channel_id.clone(),
                message_ts: "1740.301".to_string(),
            })
        }

        async fn update_working_status(
            &self,
            status: &SlackThreadStatus,
            text: &str,
        ) -> Result<()> {
            if *self.fail_updates.lock().await {
                return Err(anyhow::anyhow!("Slack API error: message_not_found"));
            }
            self.status_updates
                .lock()
                .await
                .push((status.clone(), text.to_string()));
            Ok(())
        }

        async fn delete_message(&self, _status: &SlackThreadStatus) -> Result<()> {
            Ok(())
        }

        async fn get_message_permalink(&self, channel_id: &str, message_ts: &str) -> Result<String> {
            self.permalink_requests
                .lock()
                .await
                .push((channel_id.to_string(), message_ts.to_string()));
            Ok(format!("https://example.com/{channel_id}/{message_ts}"))
        }

        async fn post_final_reply(
            &self,
            target: &SlackMessageTarget,
            text: &str,
        ) -> Result<SlackPostedMessage> {
            self.final_replies
                .lock()
                .await
                .push((target.clone(), text.to_string()));

            Ok(SlackPostedMessage {
                channel_id: target.channel_id.clone(),
                message_ts: "1740.300".to_string(),
            })
        }
    }

    #[async_trait]
    impl SlackWorkingStatusPublisher for RecordingSessionPublisher {
        async fn post_working_status(
            &self,
            target: &SlackMessageTarget,
            text: impl Into<String> + Send,
        ) -> Result<SlackThreadStatus> {
            let text = text.into();
            self.status_updates.lock().await.push((
                SlackThreadStatus {
                    channel_id: target.channel_id.clone(),
                    thread_ts: target.thread_ts.clone(),
                    status_message_ts: "1740.200".to_string(),
                },
                text,
            ));

            Ok(SlackThreadStatus {
                channel_id: target.channel_id.clone(),
                thread_ts: target.thread_ts.clone(),
                status_message_ts: "1740.200".to_string(),
            })
        }
    }

    #[test]
    fn parse_thread_reply_accepts_normal_user_thread_message() {
        let parsed = parse_thread_reply(SlackEnvelope {
            channel: Some("C123".to_string()),
            text: Some("continue".to_string()),
            thread_ts: Some("1740.100".to_string()),
            user: Some("U123".to_string()),
            bot_id: None,
            subtype: None,
        });

        assert_eq!(
            parsed,
            Some(SlackThreadReply {
                channel_id: "C123".to_string(),
                thread_ts: "1740.100".to_string(),
                text: "continue".to_string(),
                user_id: "U123".to_string(),
            })
        );
    }

    #[test]
    fn parse_thread_reply_rejects_bot_message() {
        let parsed = parse_thread_reply(SlackEnvelope {
            channel: Some("C123".to_string()),
            text: Some("continue".to_string()),
            thread_ts: Some("1740.100".to_string()),
            user: Some("U123".to_string()),
            bot_id: Some("B123".to_string()),
            subtype: None,
        });

        assert_eq!(parsed, None);
    }

    #[test]
    fn command_requests_new_session_accepts_empty_and_start() {
        assert!(command_requests_new_session(None));
        assert!(command_requests_new_session(Some("")));
        assert!(command_requests_new_session(Some("start")));
        assert!(!command_requests_new_session(Some("command")));
    }

    #[test]
    fn parse_socket_mode_request_reads_slash_command_envelope() {
        let parsed = parse_socket_mode_request(
            r#"{
              "envelope_id":"env-1",
              "type":"slash_commands",
              "accepts_response_payload":true,
              "payload":{
                "command":"/cc",
                "channel_id":"C123",
                "user_id":"U123",
                "text":"start"
              }
            }"#,
        )
        .expect("parse socket mode request");

        assert_eq!(
            parsed,
            SocketModeRequest::SlashCommand {
                envelope_id: "env-1".to_string(),
                payload: SlackSlashCommandPayload {
                    command: "/cc".to_string(),
                    channel_id: "C123".to_string(),
                    user_id: "U123".to_string(),
                    text: Some("start".to_string()),
                },
            }
        );
    }

    #[test]
    fn build_socket_mode_ack_includes_optional_payload() {
        let ack = build_socket_mode_ack("env-1", Some(json!({ "text": "Starting..." })))
            .expect("build ack");
        let payload: serde_json::Value = serde_json::from_str(&ack).expect("parse ack");

        assert_eq!(payload["envelope_id"], "env-1");
        assert_eq!(payload["payload"]["text"], "Starting...");
    }

    #[test]
    fn build_main_menu_response_matches_slack_entrypoint_contract() {
        let payload = crate::socket_mode::build_main_menu_response();

        assert_eq!(payload["text"], "Choose an action");
        assert_eq!(payload["blocks"][0]["type"], "actions");
        assert_eq!(payload["blocks"][0]["elements"][0]["action_id"], "claude_session_new");
        assert_eq!(payload["blocks"][0]["elements"][1]["action_id"], "claude_session_list");
    }

    #[test]
    fn parse_socket_mode_request_reads_block_action_envelope() {
        let parsed = parse_socket_mode_request(
            r#"{
              "envelope_id":"env-2",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.900"},
                "actions":[{"action_id":"claude_session_new","value":"claude.session.new"}]
              }
            }"#,
        )
        .expect("parse interactive request");

        assert_eq!(
            parsed,
            SocketModeRequest::Interactive {
                envelope_id: "env-2".to_string(),
                action: Some(crate::socket_mode::SlackBlockActionPayload {
                    channel_id: "C123".to_string(),
                    thread_ts: Some("1740.900".to_string()),
                    action_id: "claude_session_new".to_string(),
                    value: Some("claude.session.new".to_string()),
                    user_id: None,
                }),
            }
        );
    }

    #[tokio::test]
    async fn interactive_new_session_action_starts_session_for_channel() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&["U123".to_string()]),
            r#"{
              "envelope_id":"env-2",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "user":{"id":"U123"},
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.900"},
                "actions":[{"action_id":"claude_session_new","value":"claude.session.new"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");

        tokio::task::yield_now().await;

        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-2");
        assert_eq!(
            orchestrator.started_channels.lock().await.as_slice(),
            &["C123".to_string()]
        );
    }

    #[tokio::test]
    async fn interactive_session_list_action_only_acks() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&["U123".to_string()]),
            r#"{
              "envelope_id":"env-3",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "user":{"id":"U123"},
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.901"},
                "actions":[{"action_id":"claude_session_list","value":"claude.session.list"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");

        tokio::task::yield_now().await;

        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-3");
        assert!(orchestrator.started_channels.lock().await.is_empty());
        assert!(payload.get("payload").is_none());
        assert_eq!(
            orchestrator.posted_lists.lock().await.as_slice(),
            &[("C123".to_string(), "1740.901".to_string())]
        );
    }

    #[tokio::test]
    async fn interactive_command_palette_action_routes_to_thread_action() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&["U123".to_string()]),
            r#"{
              "envelope_id":"env-4",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "user":{"id":"U123"},
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.902"},
                "actions":[{"action_id":"claude_command_palette_open","value":"open"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");

        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-4");
        assert_eq!(
            orchestrator.actions.lock().await.as_slice(),
            &[(
                "C123".to_string(),
                "1740.902".to_string(),
                SlackThreadAction::OpenCommandPalette,
            )]
        );
    }

    #[tokio::test]
    async fn interactive_open_thread_url_action_is_treated_as_noop() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&[]),
            r#"{
              "envelope_id":"env-5",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.903"},
                "actions":[{"action_id":"claude_session_open_thread"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");

        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-5");
        assert!(orchestrator.actions.lock().await.is_empty());
        assert!(orchestrator.started_channels.lock().await.is_empty());
        assert!(orchestrator.posted_lists.lock().await.is_empty());
    }

    #[test]
    fn build_thread_message_request_targets_thread() {
        let request = build_thread_message_request(
            &SlackMessageTarget {
                channel_id: "C123".to_string(),
                thread_ts: "1740.100".to_string(),
            },
            "working",
        );

        assert_eq!(request.channel, SlackChannelId("C123".into()));
        assert_eq!(request.thread_ts, Some(SlackTs("1740.100".into())));
        assert_eq!(request.content.text, Some("working".to_string()));
    }

    #[test]
    fn build_status_update_request_targets_existing_message() {
        let request = build_status_update_request(
            &SlackPostedMessage {
                channel_id: "C123".to_string(),
                message_ts: "1740.200".to_string(),
            },
            "done",
        );

        assert_eq!(request.channel, SlackChannelId("C123".into()));
        assert_eq!(request.ts, SlackTs("1740.200".into()));
        assert_eq!(request.content.text, Some("done".to_string()));
    }

    #[test]
    fn build_status_delete_request_targets_existing_message() {
        let request = build_status_delete_request(&SlackThreadStatus {
            channel_id: "C123".to_string(),
            thread_ts: "1740.100".to_string(),
            status_message_ts: "1740.200".to_string(),
        });

        assert_eq!(request.channel, SlackChannelId("C123".into()));
        assert_eq!(request.ts, SlackTs("1740.200".into()));
    }

    #[test]
    fn slack_thread_status_keeps_thread_and_status_message_identity() {
        let status = SlackThreadStatus {
            channel_id: "C123".to_string(),
            thread_ts: "1740.100".to_string(),
            status_message_ts: "1740.200".to_string(),
        };

        assert_eq!(status.channel_id, "C123");
        assert_eq!(status.thread_ts, "1740.100");
        assert_eq!(status.status_message_ts, "1740.200");
    }

    #[test]
    fn parse_push_thread_reply_accepts_user_message_in_thread() {
        let parsed = parse_push_thread_reply(&SlackPushEventCallback {
            team_id: SlackTeamId("T123".into()),
            api_app_id: SlackAppId("A123".into()),
            event: SlackEventCallbackBody::Message(SlackMessageEvent {
                origin: SlackMessageOrigin {
                    ts: SlackTs("1740.200".into()),
                    channel: Some(SlackChannelId("C123".into())),
                    channel_type: None,
                    thread_ts: Some(SlackTs("1740.100".into())),
                    client_msg_id: None,
                },
                content: Some(SlackMessageContent {
                    text: Some("continue".into()),
                    blocks: None,
                    attachments: None,
                    upload: None,
                    files: None,
                    reactions: None,
                    metadata: None,
                }),
                sender: SlackMessageSender {
                    user: Some(SlackUserId("U123".into())),
                    bot_id: None,
                    username: None,
                    display_as_bot: None,
                    user_profile: None,
                    bot_profile: None,
                },
                subtype: None,
                hidden: None,
                message: None,
                previous_message: None,
                deleted_ts: None,
            }),
            event_id: SlackEventId("Ev123".into()),
            event_time: SlackDateTime(Utc::now()),
            event_context: None,
            authed_users: None,
            authorizations: None,
        });

        assert_eq!(
            parsed,
            Some(SlackThreadReply {
                channel_id: "C123".to_string(),
                thread_ts: "1740.100".to_string(),
                text: "continue".to_string(),
                user_id: "U123".to_string(),
            })
        );
    }

    #[test]
    fn parse_push_thread_reply_rejects_message_without_top_level_text() {
        let parsed = parse_push_thread_reply(&SlackPushEventCallback {
            team_id: SlackTeamId("T123".into()),
            api_app_id: SlackAppId("A123".into()),
            event: SlackEventCallbackBody::Message(SlackMessageEvent {
                origin: SlackMessageOrigin {
                    ts: SlackTs("1740.200".into()),
                    channel: Some(SlackChannelId("C123".into())),
                    channel_type: None,
                    thread_ts: Some(SlackTs("1740.100".into())),
                    client_msg_id: None,
                },
                content: Some(SlackMessageContent {
                    text: None,
                    blocks: None,
                    attachments: None,
                    upload: None,
                    files: None,
                    reactions: None,
                    metadata: None,
                }),
                sender: SlackMessageSender {
                    user: Some(SlackUserId("U123".into())),
                    bot_id: None,
                    username: None,
                    display_as_bot: None,
                    user_profile: None,
                    bot_profile: None,
                },
                subtype: None,
                hidden: None,
                message: Some(SlackMessageEventEdited {
                    ts: SlackTs("1740.200".into()),
                    content: Some(SlackMessageContent::new().with_text("continue".to_string())),
                    sender: SlackMessageSender {
                        user: Some(SlackUserId("U123".into())),
                        bot_id: None,
                        username: None,
                        display_as_bot: None,
                        user_profile: None,
                        bot_profile: None,
                    },
                    edited: None,
                }),
                previous_message: None,
                deleted_ts: None,
            }),
            event_id: SlackEventId("Ev123".into()),
            event_time: SlackDateTime(Utc::now()),
            event_context: None,
            authed_users: None,
            authorizations: None,
        });

        assert_eq!(parsed, None);
    }

    #[tokio::test]
    async fn handle_thread_reply_routes_text_to_bound_session() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let session_id = SessionId::new();
        store
            .insert(
                TransportBinding {
                    project_space_id: "C123".to_string(),
                    session_space_id: "1740.100".to_string(),
                },
                session_id,
            )
            .await;
        let resolver = Arc::new(RecordingResolver {
            calls: Arc::new(Mutex::new(Vec::new())),
            handle: Some(fake_handle(session_id, SessionState::Idle)),
        });
        let transport = SlackTransport::new(store, resolver.clone(), Arc::new(RecordingConfigurator::default()));

        let state = transport
            .handle_thread_reply(SlackThreadReply {
                channel_id: "C123".to_string(),
                thread_ts: "1740.100".to_string(),
                text: "continue".to_string(),
                user_id: "U123".to_string(),
            })
            .await
            .expect("route thread reply");

        assert_eq!(state, SessionState::Idle);
        assert_eq!(*resolver.calls.lock().await, vec![session_id]);
    }

    #[tokio::test]
    async fn handle_thread_reply_errors_when_binding_is_missing() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let resolver = Arc::new(RecordingResolver::default());
        let transport = SlackTransport::new(store, resolver, Arc::new(RecordingConfigurator::default()));

        let error = transport
            .handle_thread_reply(SlackThreadReply {
                channel_id: "C123".to_string(),
                thread_ts: "1740.100".to_string(),
                text: "continue".to_string(),
                user_id: "U123".to_string(),
            })
            .await
            .expect_err("missing binding should fail");

        assert!(error.to_string().contains("no session binding"));
    }

    #[tokio::test]
    async fn bind_thread_persists_binding_for_future_lookup() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let session_id = SessionId::new();
        let resolver = Arc::new(RecordingResolver {
            calls: Arc::new(Mutex::new(Vec::new())),
            handle: Some(fake_handle(session_id, SessionState::Idle)),
        });
        let transport = SlackTransport::new(
            store.clone(),
            resolver,
            Arc::new(RecordingConfigurator::default()),
        );

        transport
            .bind_thread("C999", "2000.100", session_id)
            .await
            .expect("bind thread");

        let loaded = store
            .find_session_id(&TransportBinding {
                project_space_id: "C999".to_string(),
                session_space_id: "2000.100".to_string(),
            })
            .await
            .expect("load binding");

        assert_eq!(loaded, Some(session_id));
    }

    #[tokio::test]
    async fn start_session_binds_thread_and_initializes_idle_state() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let session_id = SessionId::new();
        let resolver = Arc::new(RecordingResolver {
            calls: Arc::new(Mutex::new(Vec::new())),
            handle: Some(fake_handle(session_id, SessionState::Idle)),
        });
        let configurator = Arc::new(RecordingConfigurator::default());
        let transport = SlackTransport::new(store.clone(), resolver.clone(), configurator.clone());

        let started = transport
            .start_session(SlackSessionStart {
                channel_id: "C777".to_string(),
                thread_ts: "3000.100".to_string(),
                launch_command: "claude".to_string(),
            }, "/tmp/project")
            .await
            .expect("start session");

        let loaded = store
            .find_session_id(&TransportBinding {
                project_space_id: "C777".to_string(),
                session_space_id: "3000.100".to_string(),
            })
            .await
            .expect("load binding");

        assert_eq!(loaded, Some(started.session_id));
        assert_eq!(started.state, SessionState::Idle);
        assert_eq!(*resolver.calls.lock().await, vec![started.session_id]);
        assert_eq!(
            configurator.calls.lock().await.as_slice(),
            &[(started.session_id, "/tmp/project".to_string())]
        );
    }

    #[tokio::test]
    async fn start_session_with_working_status_persists_status_message_binding() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let resolver = Arc::new(RecordingResolver {
            calls: Arc::new(Mutex::new(Vec::new())),
            handle: Some(fake_handle(SessionId::new(), SessionState::Idle)),
        });
        let publisher = RecordingWorkingStatusPublisher::default();
        let transport = SlackTransport::new(
            store.clone(),
            resolver,
            Arc::new(RecordingConfigurator::default()),
        );

        let started = transport
            .start_session_with_working_status(
                SlackSessionStart {
                    channel_id: "C777".to_string(),
                    thread_ts: "3000.100".to_string(),
                    launch_command: "claude".to_string(),
                },
                "/tmp/project",
                &publisher,
            )
            .await
            .expect("start session with working status");

        let persisted = store
            .find_status_message(&started.binding)
            .await
            .expect("find status message")
            .expect("status message should exist");

        assert_eq!(persisted.status_message_id, "1740.200");
        assert_eq!(
            publisher.statuses.lock().await.as_slice(),
            &[(
                SlackMessageTarget {
                    channel_id: "C777".to_string(),
                    thread_ts: "3000.100".to_string(),
                },
                "⏳ Working...".to_string(),
            )]
        );
    }

    #[tokio::test]
    async fn update_working_status_uses_persisted_status_message_identity() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let binding = TransportBinding {
            project_space_id: "C777".to_string(),
            session_space_id: "3000.100".to_string(),
        };
        store
            .save_status_message(&TransportStatusMessage {
                binding: binding.clone(),
                status_message_id: "3000.200".to_string(),
            })
            .await
            .expect("save status message");
        let transport = SlackTransport::new(
            store,
            Arc::new(RecordingResolver::default()),
            Arc::new(RecordingConfigurator::default()),
        );
        let publisher = RecordingSessionPublisher::default();

        transport
            .update_working_status(&binding, &publisher, "Still working...")
            .await
            .expect("update working status");

        assert_eq!(
            publisher.status_updates.lock().await.as_slice(),
            &[(
                SlackThreadStatus {
                    channel_id: "C777".to_string(),
                    thread_ts: "3000.100".to_string(),
                    status_message_ts: "3000.200".to_string(),
                },
                "Still working...".to_string(),
            )]
        );
    }

    #[tokio::test]
    async fn ensure_working_status_reposts_when_prior_status_was_deleted() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let binding = TransportBinding {
            project_space_id: "C777".to_string(),
            session_space_id: "3000.100".to_string(),
        };
        store
            .save_status_message(&TransportStatusMessage {
                binding: binding.clone(),
                status_message_id: "3000.200".to_string(),
            })
            .await
            .expect("save status message");
        let transport = SlackTransport::new(
            store.clone(),
            Arc::new(RecordingResolver::default()),
            Arc::new(RecordingConfigurator::default()),
        );
        let publisher = RecordingWorkingStatusPublisher::default();
        let session_publisher = RecordingSessionPublisher::default();
        *session_publisher.fail_updates.lock().await = true;

        struct CombinedPublisher {
            status: RecordingWorkingStatusPublisher,
            session: RecordingSessionPublisher,
        }

        #[async_trait]
        impl SlackWorkingStatusPublisher for CombinedPublisher {
            async fn post_working_status(
                &self,
                target: &SlackMessageTarget,
                text: impl Into<String> + Send,
            ) -> Result<SlackThreadStatus> {
                self.status.post_working_status(target, text).await
            }
        }

        #[async_trait]
        impl SlackSessionPublisher for CombinedPublisher {
            async fn post_channel_message(&self, channel_id: &str, text: &str) -> Result<SlackPostedMessage> {
                self.session.post_channel_message(channel_id, text).await
            }

            async fn post_thread_message_with_blocks(
                &self,
                target: &SlackMessageTarget,
                text: &str,
                blocks: Vec<SlackBlock>,
            ) -> Result<SlackPostedMessage> {
                self.session
                    .post_thread_message_with_blocks(target, text, blocks)
                    .await
            }

            async fn update_working_status(&self, status: &SlackThreadStatus, text: &str) -> Result<()> {
                self.session.update_working_status(status, text).await
            }

            async fn delete_message(&self, status: &SlackThreadStatus) -> Result<()> {
                self.session.delete_message(status).await
            }

            async fn get_message_permalink(&self, channel_id: &str, message_ts: &str) -> Result<String> {
                self.session.get_message_permalink(channel_id, message_ts).await
            }

            async fn post_final_reply(
                &self,
                target: &SlackMessageTarget,
                text: &str,
            ) -> Result<SlackPostedMessage> {
                self.session.post_final_reply(target, text).await
            }
        }

        let publisher = CombinedPublisher {
            status: publisher.clone(),
            session: session_publisher.clone(),
        };

        transport
            .ensure_working_status(&binding, &publisher, "⏳ Working...")
            .await
            .expect("ensure working status");

        assert_eq!(
            publisher.status.statuses.lock().await.as_slice(),
            &[(
                SlackMessageTarget {
                    channel_id: "C777".to_string(),
                    thread_ts: "3000.100".to_string(),
                },
                "⏳ Working...".to_string(),
            )]
        );
    }

    #[tokio::test]
    async fn post_final_reply_targets_bound_thread() {
        let store = Arc::new(InMemorySlackBindingStore::new());
        let binding = TransportBinding {
            project_space_id: "C777".to_string(),
            session_space_id: "3000.100".to_string(),
        };
        let transport = SlackTransport::new(
            store,
            Arc::new(RecordingResolver::default()),
            Arc::new(RecordingConfigurator::default()),
        );
        let publisher = RecordingSessionPublisher::default();

        transport
            .post_final_reply(&binding, &publisher, "Finished.")
            .await
            .expect("post final reply");

        assert_eq!(
            publisher.final_replies.lock().await.as_slice(),
            &[(
                SlackMessageTarget {
                    channel_id: "C777".to_string(),
                    thread_ts: "3000.100".to_string(),
                },
                "Finished.".to_string(),
            )]
        );
    }

    #[test]
    fn build_thread_message_request_with_blocks_supports_markdown_blocks() {
        let target = SlackMessageTarget {
            channel_id: "C777".to_string(),
            thread_ts: "3000.100".to_string(),
        };
        let request = build_thread_message_request_with_blocks(
            &target,
            "Fallback text",
            vec![SlackMarkdownBlock {
                block_id: None,
                text: "**Bold**\n\n1. First".to_string(),
            }
            .into()],
        );

        let payload = serde_json::to_value(&request).expect("serialize request");

        assert_eq!(payload["blocks"][0]["type"], "markdown");
        assert_eq!(payload["blocks"][0]["text"], "**Bold**\n\n1. First");
        assert_eq!(payload["text"], "Fallback text");
    }

    #[test]
    fn to_plain_fallback_strips_slack_sensitive_markdown() {
        let text = crate::formatting::to_plain_fallback("# Summary\n\n**Bold**\n\n`code`");

        assert_eq!(text, "Summary\n\nBold\n\ncode");
    }

    // ── claude_md_to_slack_mrkdwn tests ────────────────────────────────────────

    #[test]
    fn mrkdwn_converts_bold() {
        assert_eq!(claude_md_to_slack_mrkdwn("**bold** text"), "*bold* text");
    }

    #[test]
    fn mrkdwn_converts_heading1_to_bold() {
        assert_eq!(claude_md_to_slack_mrkdwn("# 제목"), "*제목*");
    }

    #[test]
    fn mrkdwn_converts_all_heading_levels() {
        assert_eq!(claude_md_to_slack_mrkdwn("## 제목2"), "*제목2*");
        assert_eq!(claude_md_to_slack_mrkdwn("### 제목3"), "*제목3*");
    }

    #[test]
    fn mrkdwn_converts_star_list_to_bullet() {
        assert_eq!(claude_md_to_slack_mrkdwn("* 항목"), "• 항목");
    }

    #[test]
    fn mrkdwn_strips_horizontal_rule() {
        assert_eq!(claude_md_to_slack_mrkdwn("---"), "");
    }

    #[test]
    fn mrkdwn_preserves_code_block() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(claude_md_to_slack_mrkdwn(input), input);
    }

    #[test]
    fn mrkdwn_preserves_inline_code() {
        assert_eq!(claude_md_to_slack_mrkdwn("`code`"), "`code`");
    }

    #[test]
    fn mrkdwn_leaves_plain_text_unchanged() {
        assert_eq!(claude_md_to_slack_mrkdwn("일반 텍스트입니다."), "일반 텍스트입니다.");
    }

    #[test]
    fn thread_reply_carries_sender_user_id() {
        let parsed = parse_thread_reply(SlackEnvelope {
            channel: Some("C123".to_string()),
            text: Some("hello".to_string()),
            thread_ts: Some("1740.100".to_string()),
            user: Some("U456".to_string()),
            bot_id: None,
            subtype: None,
        });
        assert_eq!(parsed.unwrap().user_id, "U456");
    }

    #[test]
    fn thread_reply_from_non_allowed_user_is_blocked_by_is_allowed_user() {
        let reply = parse_thread_reply(SlackEnvelope {
            channel: Some("C123".to_string()),
            text: Some("hello".to_string()),
            thread_ts: Some("1740.100".to_string()),
            user: Some("U123".to_string()),
            bot_id: None,
            subtype: None,
        })
        .unwrap();

        let allowed = vec!["U999".to_string()];
        assert!(!is_allowed_user(&reply.user_id, &allowed));

        let allowed_with_user = vec!["U123".to_string()];
        assert!(is_allowed_user(&reply.user_id, &allowed_with_user));
    }

    #[test]
    fn parse_allowed_user_ids_returns_empty_for_blank_value() {
        assert!(parse_allowed_user_ids("").is_empty());
        assert!(parse_allowed_user_ids("  , , ").is_empty());
    }

    #[test]
    fn parse_allowed_user_ids_parses_single_id() {
        assert_eq!(parse_allowed_user_ids("U123"), vec!["U123".to_string()]);
    }

    #[test]
    fn parse_allowed_user_ids_parses_multiple_ids() {
        assert_eq!(
            parse_allowed_user_ids("U123,U456,U789"),
            vec!["U123".to_string(), "U456".to_string(), "U789".to_string()]
        );
    }

    #[test]
    fn parse_allowed_user_ids_trims_whitespace_and_skips_empty() {
        assert_eq!(
            parse_allowed_user_ids(" U123 , U456 , , U789 "),
            vec!["U123".to_string(), "U456".to_string(), "U789".to_string()]
        );
    }

    #[test]
    fn is_allowed_user_denies_all_when_list_is_empty() {
        // Fail-closed: empty allowlist must deny everyone.
        assert!(!is_allowed_user("U123", &[]));
        assert!(!is_allowed_user("", &[]));
    }

    #[test]
    fn is_allowed_user_matches_exact_id() {
        let ids = vec!["U123".to_string(), "U456".to_string()];
        assert!(is_allowed_user("U123", &ids));
        assert!(is_allowed_user("U456", &ids));
        assert!(!is_allowed_user("U999", &ids));
    }

    #[tokio::test]
    async fn slash_command_from_non_allowed_user_returns_unauthorized_ack() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());
        let allowed = vec!["U999".to_string()];

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-1",
              "type":"slash_commands",
              "accepts_response_payload":true,
              "payload":{
                "command":"/cc",
                "channel_id":"C123",
                "user_id":"U123",
                "text":"start"
              }
            }"#,
        )
        .await
        .expect("handle slash command");

        tokio::task::yield_now().await;

        let ack = ack.expect("slash command should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-1");
        assert!(
            payload["payload"]["text"]
                .as_str()
                .unwrap_or("")
                .contains("not authorized"),
            "ack payload should contain 'not authorized'"
        );
        assert!(
            orchestrator.started_channels.lock().await.is_empty(),
            "no session should be started for non-allowed user"
        );
    }

    #[tokio::test]
    async fn interactive_action_from_non_allowed_user_is_acked_but_not_processed() {
        let orchestrator = Arc::new(RecordingOrchestrator::default());
        let allowed = vec!["U999".to_string()];

        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-2",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "user":{"id":"U123"},
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.900"},
                "actions":[{"action_id":"claude_session_new","value":"claude.session.new"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");

        tokio::task::yield_now().await;

        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-2");
        assert!(
            orchestrator.started_channels.lock().await.is_empty(),
            "no session should be started for non-allowed user"
        );
    }

    // ── Scenario: 허용된 사용자의 슬래시 커맨드 ──────────────────────────────────
    //
    // Scenario: 허용된 사용자가 /cc start를 보내면 세션이 시작된다
    //   Given 허용 목록에 U123이 등록되어 있다
    //   When U123이 /cc start 슬래시 커맨드를 보낸다
    //   Then 해당 채널에 세션 시작이 요청된다
    #[tokio::test]
    async fn 허용된_사용자의_슬래시_커맨드는_세션을_시작한다() {
        // Given
        let orchestrator = Arc::new(RecordingOrchestrator::default());
        let allowed = vec!["U123".to_string()];

        // When
        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-allowed-1",
              "type":"slash_commands",
              "accepts_response_payload":true,
              "payload":{
                "command":"/cc",
                "channel_id":"C123",
                "user_id":"U123",
                "text":"start"
              }
            }"#,
        )
        .await
        .expect("handle slash command");

        tokio::task::yield_now().await;

        // Then
        let ack = ack.expect("slash command should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-allowed-1");
        assert_eq!(
            orchestrator.started_channels.lock().await.as_slice(),
            &["C123".to_string()],
            "허용된 사용자의 커맨드는 세션을 시작해야 함"
        );
    }

    // ── Scenario: 다중 허용 사용자 ───────────────────────────────────────────────
    //
    // Scenario: 여러 허용 사용자 중 하나가 커맨드를 보내도 세션이 시작된다
    //   Given 허용 목록에 U123, U456, U789가 등록되어 있다
    //   When U456이 /cc start 슬래시 커맨드를 보낸다
    //   Then 세션 시작이 요청된다
    //   And U789가 /cc start를 보내도 세션이 시작된다
    #[tokio::test]
    async fn 다중_허용_사용자_중_누구나_세션을_시작할_수_있다() {
        // Given
        let orchestrator = Arc::new(RecordingOrchestrator::default());
        let allowed = vec!["U123".to_string(), "U456".to_string(), "U789".to_string()];

        // When - 두 번째 허용 사용자
        handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-multi-1",
              "type":"slash_commands",
              "accepts_response_payload":true,
              "payload":{
                "command":"/cc",
                "channel_id":"C456",
                "user_id":"U456",
                "text":"start"
              }
            }"#,
        )
        .await
        .expect("handle U456 slash command");
        tokio::task::yield_now().await;

        // When - 세 번째 허용 사용자
        handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-multi-2",
              "type":"slash_commands",
              "accepts_response_payload":true,
              "payload":{
                "command":"/cc",
                "channel_id":"C789",
                "user_id":"U789",
                "text":"start"
              }
            }"#,
        )
        .await
        .expect("handle U789 slash command");
        tokio::task::yield_now().await;

        // Then - 두 채널 모두 세션 시작
        let started = orchestrator.started_channels.lock().await;
        assert!(started.contains(&"C456".to_string()), "U456의 채널 세션이 시작되어야 함");
        assert!(started.contains(&"C789".to_string()), "U789의 채널 세션이 시작되어야 함");
    }

    // ── Scenario: 허용된 사용자의 interactive action ─────────────────────────────
    //
    // Scenario: 허용된 사용자가 세션 시작 버튼을 누르면 세션이 시작된다
    //   Given 허용 목록에 U123이 등록되어 있다
    //   When U123이 claude_session_new 인터랙션을 보낸다
    //   Then 해당 채널에 세션 시작이 요청된다
    #[tokio::test]
    async fn 허용된_사용자의_interactive_action은_처리된다() {
        // Given
        let orchestrator = Arc::new(RecordingOrchestrator::default());
        let allowed = vec!["U123".to_string()];

        // When
        let ack = handle_socket_mode_text(
            orchestrator.clone(),
            &test_slack_config(&allowed),
            r#"{
              "envelope_id":"env-allowed-2",
              "type":"interactive",
              "payload":{
                "type":"block_actions",
                "user":{"id":"U123"},
                "channel":{"id":"C123"},
                "container":{"channel_id":"C123","message_ts":"1740.900"},
                "actions":[{"action_id":"claude_session_new","value":"claude.session.new"}]
              }
            }"#,
        )
        .await
        .expect("handle interactive request");
        tokio::task::yield_now().await;

        // Then
        let ack = ack.expect("interactive request should be acked");
        let payload: Value = serde_json::from_str(&ack).expect("parse ack");
        assert_eq!(payload["envelope_id"], "env-allowed-2");
        assert_eq!(
            orchestrator.started_channels.lock().await.as_slice(),
            &["C123".to_string()],
            "허용된 사용자의 인터랙션은 세션을 시작해야 함"
        );
    }

}
