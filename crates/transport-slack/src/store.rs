use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use core_model::{SessionId, TransportBinding, TransportStatusMessage};
use session_store::SqliteSessionRepository;
use tokio::sync::RwLock;

use crate::ports::{
    SessionBindingRegistrar, SessionBindingStore, SessionStatusRegistrar, SessionStatusStore,
    SlackSessionCatalogStore,
};
use crate::types::SlackListedSession;

pub struct InMemorySlackBindingStore {
    bindings: RwLock<HashMap<TransportBinding, SessionId>>,
    // Reverse map for O(1) find_binding lookups (kept in sync with `bindings`).
    by_session: RwLock<HashMap<SessionId, TransportBinding>>,
    statuses: RwLock<HashMap<TransportBinding, TransportStatusMessage>>,
}

impl InMemorySlackBindingStore {
    pub fn new() -> Self {
        Self {
            bindings: RwLock::new(HashMap::new()),
            by_session: RwLock::new(HashMap::new()),
            statuses: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySlackBindingStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySlackBindingStore {
    pub async fn insert(&self, binding: TransportBinding, session_id: SessionId) {
        self.by_session.write().await.insert(session_id, binding.clone());
        self.bindings.write().await.insert(binding, session_id);
    }
}

#[async_trait]
impl SessionBindingStore for InMemorySlackBindingStore {
    async fn find_session_id(&self, binding: &TransportBinding) -> Result<Option<SessionId>> {
        Ok(self.bindings.read().await.get(binding).copied())
    }

    async fn find_binding(&self, session_id: SessionId) -> Result<Option<TransportBinding>> {
        Ok(self.by_session.read().await.get(&session_id).cloned())
    }
}

#[async_trait]
impl SessionBindingRegistrar for InMemorySlackBindingStore {
    async fn save_binding(&self, binding: &TransportBinding, session_id: SessionId) -> Result<()> {
        self.insert(binding.clone(), session_id).await;
        Ok(())
    }
}

#[async_trait]
impl SessionStatusStore for InMemorySlackBindingStore {
    async fn find_status_message(
        &self,
        binding: &TransportBinding,
    ) -> Result<Option<TransportStatusMessage>> {
        Ok(self.statuses.read().await.get(binding).cloned())
    }
}

#[async_trait]
impl SessionStatusRegistrar for InMemorySlackBindingStore {
    async fn save_status_message(&self, status: &TransportStatusMessage) -> Result<()> {
        self.statuses
            .write()
            .await
            .insert(status.binding.clone(), status.clone());
        Ok(())
    }
}

#[async_trait]
impl SlackSessionCatalogStore for InMemorySlackBindingStore {
    async fn list_channel_sessions(&self, _channel_id: &str) -> Result<Vec<SlackListedSession>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl SessionBindingStore for SqliteSessionRepository {
    async fn find_session_id(&self, binding: &TransportBinding) -> Result<Option<SessionId>> {
        self.find_transport_binding_session_id(binding)
    }

    async fn find_binding(&self, session_id: SessionId) -> Result<Option<TransportBinding>> {
        self.find_transport_binding(session_id)
    }
}

#[async_trait]
impl SessionBindingRegistrar for SqliteSessionRepository {
    async fn save_binding(&self, binding: &TransportBinding, session_id: SessionId) -> Result<()> {
        self.save_transport_binding(binding, session_id)
    }
}

#[async_trait]
impl SessionStatusStore for SqliteSessionRepository {
    async fn find_status_message(
        &self,
        binding: &TransportBinding,
    ) -> Result<Option<TransportStatusMessage>> {
        self.find_transport_status_message(binding)
    }
}

#[async_trait]
impl SlackSessionCatalogStore for SqliteSessionRepository {
    async fn list_channel_sessions(&self, channel_id: &str) -> Result<Vec<SlackListedSession>> {
        let stored = SqliteSessionRepository::list_channel_sessions(self, channel_id)?;
        Ok(stored
            .into_iter()
            .map(|session| SlackListedSession {
                session_id: session.session_id,
                tmux_session_name: session.session_id.0.to_string(),
                thread_ts: session.thread_ts,
                project_label: String::new(),
                state: session.state,
            })
            .collect())
    }
}

#[async_trait]
impl SessionStatusRegistrar for SqliteSessionRepository {
    async fn save_status_message(&self, status: &TransportStatusMessage) -> Result<()> {
        self.save_transport_status_message(status)
    }
}

