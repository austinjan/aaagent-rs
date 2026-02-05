//! Session lifecycle management with caching
//!
//! Manages session lifecycle, in-memory caching, and persistence to disk.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::history::{Session, SessionConfig, TreeStore};

/// Manages session lifecycle and caching
///
/// Provides in-memory caching of sessions with optional disk persistence.
/// Sessions are stored in `data/sessions/{session_key}.json` for recovery.
pub struct SessionManager {
    /// In-memory session cache
    sessions: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>>,

    /// Storage backend for sessions
    store: Arc<dyn TreeStore>,

    /// Default configuration for new sessions
    default_config: SessionConfig,

    /// Base path for session persistence
    persistence_path: Option<PathBuf>,
}

impl SessionManager {
    /// Create a new SessionManager
    ///
    /// # Arguments
    /// * `store` - Storage backend for session data
    /// * `default_config` - Default configuration for new sessions
    /// * `persistence_path` - Optional path for session persistence (e.g., "data/sessions")
    pub fn new(
        store: Arc<dyn TreeStore>,
        default_config: SessionConfig,
        persistence_path: Option<PathBuf>,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            store,
            default_config,
            persistence_path,
        }
    }

    /// Get or create a session by key
    ///
    /// If the session exists in cache, returns it.
    /// Otherwise, attempts to restore from disk, or creates a new session.
    pub async fn get_or_create(&self, session_key: String) -> Result<Arc<RwLock<Session>>> {
        // Check cache first
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&session_key) {
                return Ok(Arc::clone(session));
            }
        }

        // Not in cache - try to restore from disk
        if let Some(ref base_path) = self.persistence_path {
            if let Ok(session) = self.restore_from_disk(&session_key, base_path).await {
                let session_arc = Arc::new(RwLock::new(session));
                let mut sessions = self.sessions.write().await;
                sessions.insert(session_key.clone(), Arc::clone(&session_arc));
                return Ok(session_arc);
            }
        }

        // Create new session
        let session = Session::new(Arc::clone(&self.store), self.default_config.clone())
            .await
            .context("Failed to create new session")?;

        let session_arc = Arc::new(RwLock::new(session));
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_key.clone(), Arc::clone(&session_arc));

        Ok(session_arc)
    }

    /// Remove a session from cache
    ///
    /// Does not delete from disk - use `delete()` for that.
    pub async fn remove(&self, session_key: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_key);
    }

    /// Persist a session to disk
    ///
    /// Saves session metadata to `{persistence_path}/{session_key}.json`.
    /// The actual tree data is stored in the TreeStore.
    pub async fn persist(&self, session_key: &str) -> Result<()> {
        let base_path = self
            .persistence_path
            .as_ref()
            .context("Persistence not enabled")?;

        let session_arc = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_key)
                .context("Session not found in cache")?
                .clone()
        };

        let session = session_arc.read().await;
        let session_path = base_path.join(format!("{}.json", session_key));

        // Create parent directory if needed
        if let Some(parent) = session_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create session directory")?;
        }

        // Serialize session metadata (just the key and config for now)
        let metadata = serde_json::json!({
            "session_key": session_key,
            "config": &session.config,
        });

        tokio::fs::write(&session_path, serde_json::to_string_pretty(&metadata)?)
            .await
            .context("Failed to write session file")?;

        Ok(())
    }

    /// Restore a session from disk
    async fn restore_from_disk(&self, session_key: &str, base_path: &PathBuf) -> Result<Session> {
        let session_path = base_path.join(format!("{}.json", session_key));

        if !session_path.exists() {
            anyhow::bail!("Session file not found");
        }

        let content = tokio::fs::read_to_string(&session_path)
            .await
            .context("Failed to read session file")?;

        let metadata: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse session metadata")?;

        let config: SessionConfig = serde_json::from_value(
            metadata
                .get("config")
                .context("Missing config in session metadata")?
                .clone(),
        )
        .context("Failed to parse session config")?;

        // Create session with stored config
        Session::new(Arc::clone(&self.store), config)
            .await
            .context("Failed to create session from stored config")
    }

    /// Delete a session from disk
    pub async fn delete(&self, session_key: &str) -> Result<()> {
        let base_path = self
            .persistence_path
            .as_ref()
            .context("Persistence not enabled")?;

        let session_path = base_path.join(format!("{}.json", session_key));

        if session_path.exists() {
            tokio::fs::remove_file(&session_path)
                .await
                .context("Failed to delete session file")?;
        }

        // Also remove from cache
        self.remove(session_key).await;

        Ok(())
    }

    /// Get count of cached sessions
    pub async fn cache_size(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Clear all sessions from cache (does not delete from disk)
    pub async fn clear_cache(&self) {
        let mut sessions = self.sessions.write().await;
        sessions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_get_or_create_new_session() {
        let store = Arc::new(MemoryStore::new());
        let manager = SessionManager::new(store, SessionConfig::default(), None);

        let session = manager
            .get_or_create("test-session".to_string())
            .await
            .unwrap();

        assert_eq!(manager.cache_size().await, 1);

        // Second call should return cached session
        let session2 = manager
            .get_or_create("test-session".to_string())
            .await
            .unwrap();

        assert_eq!(manager.cache_size().await, 1);
        assert!(Arc::ptr_eq(&session, &session2));
    }

    #[tokio::test]
    async fn test_remove_session() {
        let store = Arc::new(MemoryStore::new());
        let manager = SessionManager::new(store, SessionConfig::default(), None);

        manager
            .get_or_create("test-session".to_string())
            .await
            .unwrap();
        assert_eq!(manager.cache_size().await, 1);

        manager.remove("test-session").await;
        assert_eq!(manager.cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_persist_and_restore() {
        let temp = TempDir::new().unwrap();
        let store: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let manager = SessionManager::new(
            Arc::clone(&store),
            SessionConfig::default(),
            Some(temp.path().to_path_buf()),
        );

        // Create and persist session
        let session_key = "test-session".to_string();
        manager.get_or_create(session_key.clone()).await.unwrap();
        manager.persist(&session_key).await.unwrap();

        // Verify file exists
        let session_path = temp.path().join(format!("{}.json", session_key));
        assert!(session_path.exists());

        // Clear cache and restore
        manager.clear_cache().await;
        assert_eq!(manager.cache_size().await, 0);

        // Should restore from disk
        let restored = manager.get_or_create(session_key.clone()).await.unwrap();
        assert_eq!(manager.cache_size().await, 1);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let temp = TempDir::new().unwrap();
        let store: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let manager = SessionManager::new(
            Arc::clone(&store),
            SessionConfig::default(),
            Some(temp.path().to_path_buf()),
        );

        // Create and persist session
        let session_key = "test-session".to_string();
        manager.get_or_create(session_key.clone()).await.unwrap();
        manager.persist(&session_key).await.unwrap();

        let session_path = temp.path().join(format!("{}.json", session_key));
        assert!(session_path.exists());

        // Delete
        manager.delete(&session_key).await.unwrap();

        // Verify removed from cache and disk
        assert_eq!(manager.cache_size().await, 0);
        assert!(!session_path.exists());
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let store = Arc::new(MemoryStore::new());
        let manager = SessionManager::new(store, SessionConfig::default(), None);

        manager.get_or_create("session1".to_string()).await.unwrap();
        manager.get_or_create("session2".to_string()).await.unwrap();
        manager.get_or_create("session3".to_string()).await.unwrap();

        assert_eq!(manager.cache_size().await, 3);

        manager.clear_cache().await;
        assert_eq!(manager.cache_size().await, 0);
    }
}
