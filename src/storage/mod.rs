pub mod file_store;

use crate::history::Session;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Session summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: u32,
    pub preset: String,
}

/// Trait for session storage backends
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a new session
    async fn create_session(&self, session: &Session) -> Result<()>;

    /// Get a session by ID
    async fn get_session(&self, id: &str) -> Result<Option<Session>>;

    /// Update an existing session
    async fn update_session(&self, session: &Session) -> Result<()>;

    /// Delete a session
    async fn delete_session(&self, id: &str) -> Result<()>;

    /// List all sessions (summaries only)
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
}
