use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use core_model::{SessionId, SessionState, TransportBinding, TransportStatusMessage};
use core_service::{RuntimeEngine, SessionHandle, SessionRegistry, SessionRepository, SessionRuntimeCleanup};

use crate::types::{
    SlackListedSession, SlackMessageTarget, SlackPostedMessage, SlackProject, SlackSessionStart,
    SlackThreadAction, SlackThreadReply, SlackThreadStatus, StartedSlackSession,
};

/// Read-only access to the Slack thread → session binding table.
#[async_trait]
pub trait SessionBindingStore: Send + Sync {
    async fn find_session_id(&self, binding: &TransportBinding) -> Result<Option<SessionId>>;
    async fn find_binding(&self, session_id: SessionId) -> Result<Option<TransportBinding>>;
}

/// Write access to the Slack thread → session binding table.
#[async_trait]
pub trait SessionBindingRegistrar: Send + Sync {
    async fn save_binding(&self, binding: &TransportBinding, session_id: SessionId) -> Result<()>;
}

/// Read-only access to the per-thread status message table.
#[async_trait]
pub trait SessionStatusStore: Send + Sync {
    async fn find_status_message(
        &self,
        binding: &TransportBinding,
    ) -> Result<Option<TransportStatusMessage>>;
}

/// Write access to the per-thread status message table.
#[async_trait]
pub trait SessionStatusRegistrar: Send + Sync {
    async fn save_status_message(&self, status: &TransportStatusMessage) -> Result<()>;
}

/// Resolves a [`SessionId`] to a live [`SessionHandle`] that can receive messages.
#[async_trait]
pub trait SessionHandleResolver: Send + Sync {
    async fn resolve(&self, session_id: SessionId) -> Result<SessionHandle>;
}

/// Maps a Slack channel ID to its configured [`SlackProject`] (root path + display label).
#[async_trait]
pub trait SlackProjectLocator: Send + Sync {
    async fn find_project(&self, channel_id: &str) -> Result<Option<SlackProject>>;
}

/// Lists all sessions that belong to a Slack channel, for the session-list UI.
#[async_trait]
pub trait SlackSessionCatalogStore: Send + Sync {
    async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>>;
}

#[async_trait]
impl<T> SessionBindingStore for Arc<T>
where
    T: SessionBindingStore + Send + Sync,
{
    async fn find_session_id(&self, binding: &TransportBinding) -> Result<Option<SessionId>> {
        (**self).find_session_id(binding).await
    }

    async fn find_binding(&self, session_id: SessionId) -> Result<Option<TransportBinding>> {
        (**self).find_binding(session_id).await
    }
}

#[async_trait]
impl<T> SessionBindingRegistrar for Arc<T>
where
    T: SessionBindingRegistrar + Send + Sync,
{
    async fn save_binding(&self, binding: &TransportBinding, session_id: SessionId) -> Result<()> {
        (**self).save_binding(binding, session_id).await
    }
}

#[async_trait]
impl<T> SessionStatusStore for Arc<T>
where
    T: SessionStatusStore + Send + Sync,
{
    async fn find_status_message(
        &self,
        binding: &TransportBinding,
    ) -> Result<Option<TransportStatusMessage>> {
        (**self).find_status_message(binding).await
    }
}

#[async_trait]
impl<T> SessionStatusRegistrar for Arc<T>
where
    T: SessionStatusRegistrar + Send + Sync,
{
    async fn save_status_message(&self, status: &TransportStatusMessage) -> Result<()> {
        (**self).save_status_message(status).await
    }
}

#[async_trait]
impl<T> SessionHandleResolver for Arc<T>
where
    T: SessionHandleResolver + Send + Sync,
{
    async fn resolve(&self, session_id: SessionId) -> Result<SessionHandle> {
        (**self).resolve(session_id).await
    }
}

#[async_trait]
impl<T> SlackProjectLocator for Arc<T>
where
    T: SlackProjectLocator + Send + Sync,
{
    async fn find_project(&self, channel_id: &str) -> Result<Option<SlackProject>> {
        (**self).find_project(channel_id).await
    }
}

#[async_trait]
impl<T> SlackSessionCatalogStore for Arc<T>
where
    T: SlackSessionCatalogStore + Send + Sync,
{
    async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>> {
        (**self).list_channel_sessions(channel_id).await
    }
}

#[async_trait]
impl<T> SlackThreadRouter for Arc<T>
where
    T: SlackThreadRouter + Send + Sync,
{
    async fn route_thread_reply(&self, reply: SlackThreadReply) -> Result<SessionState> {
        (**self).route_thread_reply(reply).await
    }
}

#[async_trait]
impl<T> SlackSessionStarter for Arc<T>
where
    T: SlackSessionStarter + Send + Sync,
{
    async fn start_slack_session(&self, start: SlackSessionStart) -> Result<StartedSlackSession> {
        (**self).start_slack_session(start).await
    }
}

#[async_trait]
impl<T> SlackSessionOrchestrator for Arc<T>
where
    T: SlackSessionOrchestrator + Send + Sync,
{
    async fn start_new_session(&self, channel_id: &str, launch_command: String) -> Result<StartedSlackSession> {
        (**self).start_new_session(channel_id, launch_command).await
    }

    async fn handle_session_reply(&self, reply: SlackThreadReply) -> Result<SessionState> {
        (**self).handle_session_reply(reply).await
    }

    async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>> {
        (**self).list_channel_sessions(channel_id).await
    }

    async fn post_session_list(&self, channel_id: &str, thread_ts: &str) -> Result<()> {
        (**self).post_session_list(channel_id, thread_ts).await
    }

    async fn handle_thread_action(
        &self,
        channel_id: &str,
        thread_ts: &str,
        action: SlackThreadAction,
    ) -> Result<Option<SessionState>> {
        (**self)
            .handle_thread_action(channel_id, thread_ts, action)
            .await
    }
}

#[async_trait]
impl<R, E> SessionHandleResolver for SessionRegistry<R, E>
where
    R: SessionRepository + Send + Sync + 'static,
    E: RuntimeEngine + SessionRuntimeCleanup + Send + Sync + 'static,
{
    async fn resolve(&self, session_id: SessionId) -> Result<SessionHandle> {
        Ok(self.session(session_id).await)
    }
}

/// Routes inbound Slack thread messages to the bound session.
#[async_trait]
pub trait SlackThreadRouter: Send + Sync {
    async fn route_thread_reply(&self, reply: SlackThreadReply) -> Result<SessionState>;
}

/// Creates a new agent session from a Slack thread.
#[async_trait]
pub trait SlackSessionStarter: Send + Sync {
    async fn start_slack_session(&self, start: SlackSessionStart) -> Result<StartedSlackSession>;
}

/// High-level coordinator: owns the full session lifecycle from the Slack side.
///
/// Implemented by `SlackApplicationService` in the `application` crate.
/// `transport-slack` defines this trait so the socket-mode handler can depend on it
/// without pulling in application-layer logic.
#[async_trait]
pub trait SlackSessionOrchestrator: Send + Sync {
    async fn start_new_session(&self, channel_id: &str, launch_command: String) -> Result<StartedSlackSession>;
    async fn handle_session_reply(&self, reply: SlackThreadReply) -> Result<SessionState>;
    async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>>;
    async fn post_session_list(&self, channel_id: &str, thread_ts: &str) -> Result<()>;
    async fn handle_thread_action(
        &self,
        channel_id: &str,
        thread_ts: &str,
        action: SlackThreadAction,
    ) -> Result<Option<SessionState>>;
}

/// Slack Web API calls for posting, updating, and deleting messages.
#[async_trait]
pub trait SlackSessionPublisher: Send + Sync {
    async fn post_channel_message(&self, channel_id: &str, text: &str) -> Result<SlackPostedMessage>;
    async fn post_thread_message_with_blocks(
        &self,
        target: &SlackMessageTarget,
        text: &str,
        blocks: Vec<slack_morphism::prelude::SlackBlock>,
    ) -> Result<SlackPostedMessage>;
    async fn update_working_status(&self, status: &SlackThreadStatus, text: &str) -> Result<()>;
    async fn delete_message(&self, status: &SlackThreadStatus) -> Result<()>;
    async fn get_message_permalink(&self, channel_id: &str, message_ts: &str) -> Result<String>;
    async fn post_final_reply(
        &self,
        target: &SlackMessageTarget,
        text: &str,
    ) -> Result<SlackPostedMessage>;
}

/// Posts the initial "Working…" status bubble for a turn.
///
/// Separated from [`SlackSessionPublisher`] so callers that only need to post
/// the initial status don't have to satisfy the full publisher bound.
#[async_trait]
pub trait SlackWorkingStatusPublisher: Send + Sync {
    async fn post_working_status(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String> + Send,
    ) -> Result<SlackThreadStatus>;
}

/// Blanket supertrait combining post + update/delete for the status bubble lifecycle.
pub trait SlackStatusMessagePublisher: SlackSessionPublisher + SlackWorkingStatusPublisher {}

impl<T> SlackStatusMessagePublisher for T where T: SlackSessionPublisher + SlackWorkingStatusPublisher {}

