use std::sync::Arc;

use anyhow::Result;
use core_model::AgentType;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use slack_morphism::prelude::*;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::formatting::parse_push_thread_reply;
use crate::ports::SlackSessionOrchestrator;
use crate::publisher::build_slack_https_connector;
use crate::types::SlackThreadAction;

pub fn parse_allowed_user_ids(env_value: &str) -> Vec<String> {
    env_value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

pub fn is_allowed_user(user_id: &str, allowed: &[String]) -> bool {
    // Fail-closed: an empty allowlist denies everyone. Callers must ensure a
    // non-empty allowlist is configured (enforced at startup by from_env()).
    !allowed.is_empty() && allowed.iter().any(|id| id == user_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackSocketModeConfig {
    pub bot_token: String,
    pub app_token: String,
    pub allowed_user_ids: Vec<String>,
    /// Path to the hook settings JSON file (used to build non-Claude agent launch commands).
    pub hook_settings_path: String,
    /// Full launch command for Claude Code — honours `RCC_CLAUDE_COMMAND` if set.
    pub claude_launch_command: String,
}

impl SlackSocketModeConfig {
    pub fn from_env() -> Result<Self> {
        let allowed_user_ids = std::env::var("SLACK_ALLOWED_USER_ID")
            .map(|v| parse_allowed_user_ids(&v))
            .unwrap_or_default();
        if allowed_user_ids.is_empty() {
            anyhow::bail!("SLACK_ALLOWED_USER_ID is not set or empty — set at least one allowed Slack user ID");
        }
        let hook_settings_path = std::env::var("RCC_HOOK_SETTINGS_PATH")
            .unwrap_or_else(|_| ".claude/claude-stop-hooks.json".to_string());
        let claude_launch_command = std::env::var("RCC_CLAUDE_COMMAND").unwrap_or_else(|_| {
            format!("claude --settings {hook_settings_path} --dangerously-skip-permissions")
        });
        Ok(Self {
            bot_token: std::env::var("SLACK_BOT_TOKEN")?,
            app_token: std::env::var("SLACK_APP_TOKEN")?,
            allowed_user_ids,
            hook_settings_path,
            claude_launch_command,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SlackSlashCommandPayload {
    pub command: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlackBlockActionPayload {
    pub(crate) channel_id: String,
    pub(crate) thread_ts: Option<String>,
    pub(crate) action_id: String,
    pub(crate) value: Option<String>,
    pub(crate) user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInteractiveUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct RawInteractivePayload {
    #[serde(rename = "type")]
    kind: String,
    user: Option<RawInteractiveUser>,
    channel: Option<RawInteractiveChannel>,
    message: Option<RawInteractiveMessage>,
    container: Option<RawInteractiveContainer>,
    actions: Option<Vec<RawInteractiveAction>>,
}

#[derive(Debug, Deserialize)]
struct RawInteractiveChannel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct RawInteractiveMessage {
    thread_ts: Option<String>,
    ts: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInteractiveContainer {
    channel_id: Option<String>,
    message_ts: Option<String>,
    // Present in Slack payloads when the action originates from a threaded message.
    thread_ts: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawInteractiveAction {
    action_id: String,
    value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SocketModeRequest {
    Hello { app_id: String, num_connections: u32 },
    Disconnect { reason: String },
    SlashCommand {
        envelope_id: String,
        payload: SlackSlashCommandPayload,
    },
    EventsApi {
        envelope_id: String,
        payload: Box<SlackPushEventCallback>,
    },
    Interactive {
        envelope_id: String,
        action: Option<SlackBlockActionPayload>,
    },
    Unknown {
        envelope_id: Option<String>,
        kind: String,
    },
}

#[derive(Debug, Deserialize)]
struct RawSocketModeEnvelope {
    #[serde(rename = "type")]
    kind: String,
    envelope_id: Option<String>,
    payload: Option<Value>,
    connection_info: Option<RawSocketModeConnectionInfo>,
    num_connections: Option<u32>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSocketModeConnectionInfo {
    app_id: String,
}

pub(crate) fn parse_socket_mode_request(raw: &str) -> Result<SocketModeRequest> {
    let envelope: RawSocketModeEnvelope = serde_json::from_str(raw)?;

    match envelope.kind.as_str() {
        "hello" => Ok(SocketModeRequest::Hello {
            app_id: envelope
                .connection_info
                .ok_or_else(|| anyhow::anyhow!("missing connection_info for hello event"))?
                .app_id,
            num_connections: envelope.num_connections.unwrap_or(0),
        }),
        "disconnect" => Ok(SocketModeRequest::Disconnect {
            reason: envelope.reason.unwrap_or_else(|| "unknown".to_string()),
        }),
        "slash_commands" => Ok(SocketModeRequest::SlashCommand {
            envelope_id: envelope
                .envelope_id
                .ok_or_else(|| anyhow::anyhow!("missing envelope_id for slash command"))?,
            payload: serde_json::from_value(
                envelope
                    .payload
                    .ok_or_else(|| anyhow::anyhow!("missing slash command payload"))?,
            )?,
        }),
        "events_api" => Ok(SocketModeRequest::EventsApi {
            envelope_id: envelope
                .envelope_id
                .ok_or_else(|| anyhow::anyhow!("missing envelope_id for events_api"))?,
            payload: Box::new(serde_json::from_value(
                envelope
                    .payload
                    .ok_or_else(|| anyhow::anyhow!("missing events_api payload"))?,
            )?),
        }),
        "interactive" => Ok(SocketModeRequest::Interactive {
            envelope_id: envelope
                .envelope_id
                .ok_or_else(|| anyhow::anyhow!("missing envelope_id for interactive event"))?,
            action: parse_interactive_action(envelope.payload)?,
        }),
        other => Ok(SocketModeRequest::Unknown {
            envelope_id: envelope.envelope_id,
            kind: other.to_string(),
        }),
    }
}

pub(crate) fn build_socket_mode_ack(envelope_id: &str, payload: Option<Value>) -> Result<String> {
    let mut body = json!({
        "envelope_id": envelope_id,
    });

    if let Some(payload) = payload {
        body["payload"] = payload;
    }

    Ok(serde_json::to_string(&body)?)
}

fn parse_interactive_action(payload: Option<Value>) -> Result<Option<SlackBlockActionPayload>> {
    let Some(payload) = payload else {
        return Ok(None);
    };
    let payload: RawInteractivePayload = serde_json::from_value(payload)?;
    if payload.kind != "block_actions" {
        return Ok(None);
    }

    let channel_id = payload
        .channel
        .as_ref()
        .map(|channel| channel.id.clone())
        .or_else(|| payload.container.as_ref().and_then(|container| container.channel_id.clone()));
    let Some(channel_id) = channel_id else {
        return Ok(None);
    };
    let Some(action) = payload.actions.and_then(|mut actions| actions.drain(..).next()) else {
        return Ok(None);
    };

    // Priority: message.thread_ts > container.thread_ts > message.ts > container.message_ts.
    // container.thread_ts is present in Slack payloads when the action originates from a
    // threaded message, providing a reliable fallback when message.thread_ts is absent.
    let thread_ts = {
        let from_message_thread = payload.message.as_ref().and_then(|m| m.thread_ts.clone());
        let from_container_thread = payload.container.as_ref().and_then(|c| c.thread_ts.clone());
        let from_message_ts = payload.message.as_ref().and_then(|m| m.ts.clone());
        let from_container_message = payload.container.and_then(|c| c.message_ts);
        from_message_thread
            .or(from_container_thread)
            .or(from_message_ts)
            .or(from_container_message)
    };

    if thread_ts.is_none() {
        tracing::warn!(action_id = action.action_id, channel_id, "interactive action has no resolvable thread_ts; cannot route to session");
    }

    Ok(Some(SlackBlockActionPayload {
        channel_id,
        thread_ts,
        action_id: action.action_id,
        value: action.value,
        user_id: payload.user.map(|u| u.id),
    }))
}

pub(crate) fn build_main_menu_response() -> Value {
    json!({
        "text": "Choose an action",
        "blocks": [
            {
                "type": "actions",
                "elements": [
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "Start new session" },
                        "action_id": "claude_session_new",
                        "value": "claude.session.new"
                    },
                    {
                        "type": "button",
                        "text": { "type": "plain_text", "text": "View existing sessions" },
                        "action_id": "claude_session_list",
                        "value": "claude.session.list"
                    }
                ]
            }
        ]
    })
}

pub async fn serve_socket_mode(
    orchestrator: Arc<dyn SlackSessionOrchestrator>,
    config: SlackSocketModeConfig,
) -> Result<()> {
    let client = Arc::new(SlackClient::new(build_slack_https_connector()));
    let app_token: SlackApiToken = SlackApiToken::new(config.app_token.clone().into());
    let session = client.open_session(&app_token);

    tracing::info!("socket mode token registered");

    // Reconnect delay grows exponentially on repeated failures.
    let mut reconnect_delay = Duration::from_secs(1);
    const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
    // After this many consecutive open-connection failures the process gives up,
    // preventing an invisible infinite retry loop when the app has a permanent
    // misconfiguration that is not covered by the auth-string allowlist.
    const MAX_CONSECUTIVE_OPEN_FAILURES: u32 = 10;
    let mut consecutive_open_failures: u32 = 0;

    loop {
        // Request a fresh WebSocket URL from Slack. Known auth errors are fatal
        // immediately; other failures are retried up to MAX_CONSECUTIVE_OPEN_FAILURES
        // times before being treated as fatal.
        let open = match session
            .apps_connections_open(&SlackApiAppsConnectionOpenRequest::new())
            .await
        {
            Ok(open) => {
                consecutive_open_failures = 0;
                open
            }
            Err(error) => {
                let msg = error.to_string();
                if msg.contains("invalid_auth")
                    || msg.contains("not_authed")
                    || msg.contains("token_revoked")
                {
                    return Err(anyhow::anyhow!("Slack auth error (not retrying): {error}"));
                }
                consecutive_open_failures += 1;
                if consecutive_open_failures >= MAX_CONSECUTIVE_OPEN_FAILURES {
                    return Err(anyhow::anyhow!(
                        "Slack connection failed after {MAX_CONSECUTIVE_OPEN_FAILURES} consecutive attempts: {error}"
                    ));
                }
                tracing::warn!(
                    error = %error,
                    consecutive_open_failures,
                    reconnect_delay_secs = reconnect_delay.as_secs(),
                    "failed to open socket mode connection; retrying"
                );
                sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
                continue;
            }
        };

        let socket_url = open.url.0.to_string();
        tracing::debug!("opening socket mode websocket");

        let (mut stream, _response) = match connect_async(socket_url.as_str()).await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    reconnect_delay_secs = reconnect_delay.as_secs(),
                    "failed to connect websocket; retrying"
                );
                sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
                continue;
            }
        };

        // Successful connection — reset backoff.
        reconnect_delay = Duration::from_secs(1);
        tracing::info!("socket mode listener started");

        while let Some(message) = stream.next().await {
            match message {
                Ok(Message::Text(body)) => {
                    match handle_socket_mode_text(Arc::clone(&orchestrator), &config, body.as_ref()).await {
                        Ok(Some(reply)) => {
                            if let Err(error) = stream.send(Message::Text(reply.into())).await {
                                tracing::warn!(error = %error, "failed to send ack; reconnecting");
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(error = %error, "non-fatal socket mode handler error");
                        }
                    }
                }
                Ok(Message::Ping(body)) => {
                    if let Err(error) = stream.send(Message::Pong(body)).await {
                        tracing::warn!(error = %error, "failed to send pong; reconnecting");
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    tracing::info!(frame = ?frame, "socket mode websocket closed");
                    break;
                }
                Ok(Message::Binary(_)) => {}
                Ok(Message::Pong(_)) => {}
                Ok(Message::Frame(_)) => {}
                Err(error) => {
                    tracing::error!(error = ?error, "socket mode websocket error");
                    break;
                }
            }
        }

        tracing::info!("reconnecting socket mode websocket");
        reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        sleep(reconnect_delay).await;
    }
}

pub(crate) async fn handle_socket_mode_text(
    orchestrator: Arc<dyn SlackSessionOrchestrator>,
    config: &SlackSocketModeConfig,
    raw: &str,
) -> Result<Option<String>> {
    let allowed_user_ids = &config.allowed_user_ids;
    match parse_socket_mode_request(raw) {
        Ok(SocketModeRequest::Hello {
            app_id,
            num_connections,
        }) => {
            tracing::debug!(app_id, num_connections, "received socket mode hello");
            Ok(None)
        }
        Ok(SocketModeRequest::Disconnect { reason }) => {
            tracing::info!(reason, "received socket mode disconnect");
            Ok(None)
        }
        Ok(SocketModeRequest::SlashCommand {
            envelope_id,
            payload,
        }) => {
            tracing::info!(
                command = payload.command,
                channel_id = payload.channel_id,
                user_id = payload.user_id,
                text = ?payload.text,
                "received slash command"
            );

            if !is_allowed_user(&payload.user_id, allowed_user_ids) {
                tracing::warn!(user_id = payload.user_id, "slash command from non-allowed user");
                return Ok(Some(build_socket_mode_ack(
                    &envelope_id,
                    Some(json!({ "text": "You are not authorized to use this command." })),
                )?));
            }

            let command_text = payload.text.as_deref().map(str::trim).unwrap_or("");
            let agent_type = AgentType::from_slash_command(&payload.command);
            let launch_command = match agent_type {
                AgentType::ClaudeCode => config.claude_launch_command.clone(),
                AgentType::Codex => "codex".to_string(),
                AgentType::Gemini => "gemini".to_string(),
            };

            // /cc with no argument → show menu (the menu button starts Claude Code).
            // /cx or /gm with no argument → start immediately (no agent-generic menu yet).
            let should_start = command_requests_new_session(payload.text.as_deref())
                || (command_text.is_empty() && agent_type != AgentType::ClaudeCode);

            let ack_payload = if command_text.is_empty() && agent_type == AgentType::ClaudeCode {
                build_main_menu_response()
            } else if !should_start {
                json!({
                    "text": "Unsupported command. Use `/cc start`, `/cx start`, or `/gm start`."
                })
            } else {
                let channel_id = payload.channel_id.clone();
                let orchestrator = Arc::clone(&orchestrator);
                tokio::spawn(async move {
                    if let Err(error) = orchestrator.start_new_session(&channel_id, launch_command).await {
                        tracing::error!(channel_id, error = %error, "failed to start Slack session");
                    }
                });

                json!({
                    "text": format!("Starting a new {} session. Watch this channel for the new thread.", agent_type.display_name())
                })
            };

            Ok(Some(build_socket_mode_ack(&envelope_id, Some(ack_payload))?))
        }
        Ok(SocketModeRequest::EventsApi {
            envelope_id,
            payload,
        }) => {
            if let Some(reply) = parse_push_thread_reply(&payload) {
                if !is_allowed_user(&reply.user_id, allowed_user_ids) {
                    tracing::warn!(user_id = reply.user_id, "thread reply from non-allowed user ignored");
                } else if let Err(error) = orchestrator.handle_session_reply(reply).await {
                    tracing::warn!(error = %error, "failed to handle Slack thread reply");
                }
            }

            Ok(Some(build_socket_mode_ack(&envelope_id, None)?))
        }
        Ok(SocketModeRequest::Interactive {
            envelope_id,
            action,
        }) => {
            if let Some(action) = action {
                tracing::info!(
                    action_id = action.action_id,
                    channel_id = action.channel_id,
                    thread_ts = ?action.thread_ts,
                    value = ?action.value,
                    user_id = ?action.user_id,
                    "received interactive action"
                );

                let Some(action_user_id) = action.user_id.as_deref() else {
                    tracing::warn!(action_id = action.action_id, "interactive action has no user_id; ignoring");
                    return Ok(Some(build_socket_mode_ack(&envelope_id, None)?));
                };
                if !is_allowed_user(action_user_id, allowed_user_ids) {
                    tracing::warn!(user_id = action_user_id, "interactive action from non-allowed user");
                    return Ok(Some(build_socket_mode_ack(&envelope_id, None)?));
                }

                match action.action_id.as_str() {
                    "claude_session_new" => {
                        let channel_id = action.channel_id;
                        let orchestrator = Arc::clone(&orchestrator);
                        // Interactive button always starts Claude Code. Honour RCC_CLAUDE_COMMAND.
                        let launch_command = config.claude_launch_command.clone();
                        tokio::spawn(async move {
                            if let Err(error) = orchestrator.start_new_session(&channel_id, launch_command).await {
                                tracing::error!(
                                    channel_id,
                                    error = %error,
                                    "failed to start Slack session from interactive action"
                                );
                            }
                        });
                    }
                    "claude_session_list" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .post_session_list(&action.channel_id, &thread_ts)
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to post session list"
                                );
                            }
                        } else {
                            tracing::warn!(
                                channel_id = action.channel_id,
                                "interactive session list missing thread context"
                            );
                        }
                    }
                    "claude_command_palette_open" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::OpenCommandPalette,
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to open command palette"
                                );
                            }
                        }
                    }
                    "claude_command_key_interrupt" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::Interrupt,
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to interrupt session"
                                );
                            }
                        }
                    }
                    "claude_terminal_key_escape" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::SendKey {
                                        key: action
                                            .value
                                            .clone()
                                            .unwrap_or_else(|| "Escape".to_string()),
                                    },
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to send key to session"
                                );
                            }
                        }
                    }
                    "claude_command_send_clear" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::SendCommand {
                                        text: action
                                            .value
                                            .clone()
                                            .unwrap_or_else(|| "/clear".to_string()),
                                    },
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to send clear command"
                                );
                            }
                        }
                    }
                    "claude_command_send_revise_claude_md" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::SendCommand {
                                        text: action.value.clone().unwrap_or_else(|| {
                                            "/claude-md-management:revise-claude-md".to_string()
                                        }),
                                    },
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to send CLAUDE.md update command"
                                );
                            }
                        }
                    }
                    "claude_session_terminate" => {
                        if let Some(thread_ts) = action.thread_ts {
                            if let Err(error) = orchestrator
                                .handle_thread_action(
                                    &action.channel_id,
                                    &thread_ts,
                                    SlackThreadAction::Terminate,
                                )
                                .await
                            {
                                tracing::warn!(
                                    channel_id = action.channel_id,
                                    thread_ts,
                                    error = %error,
                                    "failed to terminate session"
                                );
                            }
                        }
                    }
                    "claude_session_open_thread" => {}
                    other => {
                        tracing::debug!(action_id = other, "ignored interactive action");
                    }
                }
            }
            Ok(Some(build_socket_mode_ack(&envelope_id, None)?))
        }
        Ok(SocketModeRequest::Unknown { envelope_id, kind }) => {
            tracing::debug!(kind, "ignored socket mode event type");
            match envelope_id {
                Some(envelope_id) => Ok(Some(build_socket_mode_ack(&envelope_id, None)?)),
                None => Ok(None),
            }
        }
        Err(error) => {
            tracing::error!(error = %error, raw, "failed to parse socket mode payload");
            Ok(None)
        }
    }
}

pub(crate) fn command_requests_new_session(text: Option<&str>) -> bool {
    match text.map(str::trim) {
        None | Some("") | Some("start") => true,
        Some(_) => false,
    }
}
