use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use core_model::{SessionId, SessionMsg, SessionState, TransportBinding, TransportStatusMessage, UserCommand};
use core_service::SessionRuntimeConfigurator;

use crate::ports::{
    SessionBindingRegistrar, SessionBindingStore, SessionHandleResolver, SessionStatusRegistrar,
    SessionStatusStore, SlackSessionCatalogStore, SlackSessionPublisher, SlackSessionStarter,
    SlackStatusMessagePublisher, SlackThreadRouter, SlackWorkingStatusPublisher,
};
use crate::types::{
    SlackListedSession, SlackMessageTarget, SlackPostedMessage, SlackSessionStart, SlackThreadAction,
    SlackThreadReply, SlackThreadStatus, StartedSlackSession,
};

pub struct SlackTransport<S, R, C> {
    store: Arc<S>,
    resolver: Arc<R>,
    configurator: Arc<C>,
}

impl<S, R, C> SlackTransport<S, R, C>
where
    S: SessionBindingStore,
    R: SessionHandleResolver,
    C: SessionRuntimeConfigurator,
{
    pub fn new(store: Arc<S>, resolver: Arc<R>, configurator: Arc<C>) -> Self {
        Self {
            store,
            resolver,
            configurator,
        }
    }

    pub fn configurator(&self) -> &Arc<C> {
        &self.configurator
    }

    pub async fn handle_thread_reply(&self, reply: SlackThreadReply) -> Result<SessionState> {
        let binding = TransportBinding {
            project_space_id: reply.channel_id,
            session_space_id: reply.thread_ts,
        };
        self.send_session_message(
            &binding,
            SessionMsg::UserCommand(UserCommand { text: reply.text }),
        )
        .await
    }

    pub async fn handle_thread_action(
        &self,
        channel_id: &str,
        thread_ts: &str,
        action: SlackThreadAction,
    ) -> Result<SessionState> {
        let binding = TransportBinding {
            project_space_id: channel_id.to_string(),
            session_space_id: thread_ts.to_string(),
        };
        let message = match action {
            SlackThreadAction::OpenCommandPalette => {
                return self
                    .store
                    .find_session_id(&binding)
                    .await?
                    .map(|_| SessionState::Idle)
                    .ok_or_else(|| anyhow::anyhow!("no session binding for Slack thread"));
            }
            SlackThreadAction::Interrupt => SessionMsg::Interrupt,
            SlackThreadAction::SendKey { key } => SessionMsg::SendKey { key },
            SlackThreadAction::SendCommand { text } => SessionMsg::UserCommand(UserCommand { text }),
            SlackThreadAction::Terminate => SessionMsg::Terminate,
        };
        self.send_session_message(&binding, message).await
    }

    async fn send_session_message(
        &self,
        binding: &TransportBinding,
        message: SessionMsg,
    ) -> Result<SessionState> {
        let session_id = self
            .store
            .find_session_id(binding)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no session binding for Slack thread"))?;
        let handle = self.resolver.resolve(session_id).await?;
        handle.send(message).await
    }

    pub async fn bind_thread(
        &self,
        channel_id: impl Into<String>,
        thread_ts: impl Into<String>,
        session_id: SessionId,
    ) -> Result<()>
    where
        S: SessionBindingRegistrar,
    {
        self.store
            .save_binding(
                &TransportBinding {
                    project_space_id: channel_id.into(),
                    session_space_id: thread_ts.into(),
                },
                session_id,
            )
            .await
    }

    pub async fn start_session(
        &self,
        start: SlackSessionStart,
        project_root: &str,
    ) -> Result<StartedSlackSession>
    where
        S: SessionBindingRegistrar,
    {
        let session_id = SessionId::new();
        let binding = TransportBinding {
            project_space_id: start.channel_id,
            session_space_id: start.thread_ts,
        };

        self.store.save_binding(&binding, session_id).await?;
        self.configurator
            .register_project_root(session_id, project_root)
            .await?;
        let handle = self.resolver.resolve(session_id).await?;
        let state = handle.send(SessionMsg::Recover { launch_command: start.launch_command }).await?;

        Ok(StartedSlackSession {
            session_id,
            state,
            binding,
        })
    }

    pub async fn start_session_with_working_status<P>(
        &self,
        start: SlackSessionStart,
        project_root: &str,
        publisher: &P,
    ) -> Result<StartedSlackSession>
    where
        S: SessionBindingRegistrar + SessionStatusRegistrar,
        P: SlackWorkingStatusPublisher,
    {
        let started = self.start_session(start, project_root).await?;
        let target = SlackMessageTarget {
            channel_id: started.binding.project_space_id.clone(),
            thread_ts: started.binding.session_space_id.clone(),
        };
        let status = publisher.post_working_status(&target, "⏳ Working...").await?;

        self.store
            .save_status_message(&TransportStatusMessage {
                binding: started.binding.clone(),
                status_message_id: status.status_message_ts,
            })
            .await?;

        Ok(started)
    }

    pub async fn update_working_status<P>(
        &self,
        binding: &TransportBinding,
        publisher: &P,
        text: &str,
    ) -> Result<()>
    where
        S: SessionStatusStore,
        P: SlackSessionPublisher,
    {
        let status = self
            .store
            .find_status_message(binding)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no working status recorded for Slack thread"))?;

        publisher
            .update_working_status(
                &SlackThreadStatus {
                    channel_id: status.binding.project_space_id,
                    thread_ts: status.binding.session_space_id,
                    status_message_ts: status.status_message_id,
                },
                text,
            )
            .await
    }

    pub async fn ensure_working_status<P>(
        &self,
        binding: &TransportBinding,
        publisher: &P,
        text: &str,
    ) -> Result<()>
    where
        S: SessionStatusStore + SessionStatusRegistrar,
        P: SlackStatusMessagePublisher,
    {
        if let Some(status) = self.store.find_status_message(binding).await? {
            let update_ok = publisher
                .update_working_status(
                    &SlackThreadStatus {
                        channel_id: status.binding.project_space_id.clone(),
                        thread_ts: status.binding.session_space_id.clone(),
                        status_message_ts: status.status_message_id.clone(),
                    },
                    text,
                )
                .await
                .inspect_err(|e| tracing::warn!(error = %e, "working status update failed, will repost"))
                .is_ok();
            if update_ok {
                return Ok(());
            }
        }

        let posted = publisher
            .post_working_status(
                &SlackMessageTarget {
                    channel_id: binding.project_space_id.clone(),
                    thread_ts: binding.session_space_id.clone(),
                },
                text.to_string(),
            )
            .await?;
        self.store
            .save_status_message(&TransportStatusMessage {
                binding: binding.clone(),
                status_message_id: posted.status_message_ts,
            })
            .await?;
        Ok(())
    }

    pub async fn post_final_reply<P>(
        &self,
        binding: &TransportBinding,
        publisher: &P,
        text: &str,
    ) -> Result<SlackPostedMessage>
    where
        P: SlackSessionPublisher,
    {
        publisher
            .post_final_reply(
                &SlackMessageTarget {
                    channel_id: binding.project_space_id.clone(),
                    thread_ts: binding.session_space_id.clone(),
                },
                text,
            )
            .await
    }

    pub async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>>
    where
        S: SlackSessionCatalogStore,
    {
        self.store.list_channel_sessions(channel_id).await
    }
}

#[async_trait]
impl<S, R, C> SlackThreadRouter for SlackTransport<S, R, C>
where
    S: SessionBindingStore + Send + Sync,
    R: SessionHandleResolver + Send + Sync,
    C: SessionRuntimeConfigurator + Send + Sync,
{
    async fn route_thread_reply(&self, reply: SlackThreadReply) -> Result<SessionState> {
        self.handle_thread_reply(reply).await
    }
}

#[async_trait]
impl<S, R, C> SlackSessionStarter for SlackTransport<S, R, C>
where
    S: SessionBindingStore + SessionBindingRegistrar + Send + Sync,
    R: SessionHandleResolver + Send + Sync,
    C: SessionRuntimeConfigurator + Send + Sync,
{
    async fn start_slack_session(&self, start: SlackSessionStart) -> Result<StartedSlackSession> {
        self.start_session(start, ".").await
    }
}
