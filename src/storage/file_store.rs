use super::{SessionStore, SessionSummary};
use crate::history::Session;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

/// File-based session storage
pub struct FileSessionStore {
    base_path: PathBuf,
}

impl FileSessionStore {
    /// Create a new file store at the given path
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        std::fs::create_dir_all(&base_path).context(format!(
            "Failed to create sessions directory: {}",
            base_path.display()
        ))?;
        Ok(Self { base_path })
    }

    /// Get the file path for a session
    fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{}.json", id))
    }
}

#[async_trait::async_trait]
impl SessionStore for FileSessionStore {
    async fn create_session(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.session_id);
        let json = serde_json::to_string_pretty(&session).context("Failed to serialize session")?;
        fs::write(&path, json)
            .await
            .context(format!("Failed to write session file: {}", path.display()))?;
        Ok(())
    }

    async fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let path = self.session_path(id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .await
            .context(format!("Failed to read session file: {}", path.display()))?;

        let session: Session =
            serde_json::from_str(&content).context("Failed to deserialize session")?;

        Ok(Some(session))
    }

    async fn update_session(&self, session: &Session) -> Result<()> {
        // Same as create - overwrites the file
        self.create_session(session).await
    }

    async fn delete_session(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);

        if path.exists() {
            fs::remove_file(&path)
                .await
                .context(format!("Failed to delete session file: {}", path.display()))?;
        }

        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();

        let mut entries = fs::read_dir(&self.base_path).await.context(format!(
            "Failed to read sessions directory: {}",
            self.base_path.display()
        ))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process .json files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Read and parse session
            if let Ok(content) = fs::read_to_string(&path).await {
                if let Ok(session) = serde_json::from_str::<Session>(&content) {
                    // Extract preset from metadata
                    let preset = session
                        .metadata
                        .as_ref()
                        .and_then(|m| m.get("preset"))
                        .and_then(|p| p.as_str())
                        .unwrap_or("general")
                        .to_string();

                    // Extract name from metadata or use default
                    let name = session
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Session {}", &session.session_id[..8]));

                    sessions.push(SessionSummary {
                        session_id: session.session_id.clone(),
                        name,
                        created_at: session.created_at,
                        updated_at: session.updated_at,
                        message_count: session.stats.total_nodes, // Approximate with total nodes
                        preset,
                    });
                }
            }
        }

        // Sort by updated_at (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{MemoryStore, SessionConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_session(name: &str) -> Session {
        let store = Arc::new(MemoryStore::new());
        let mut session = Session::new(store, SessionConfig::default()).await.unwrap();
        session.name = Some(name.to_string());
        session
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();

        let session = create_test_session("Test Session").await;
        let session_id = session.session_id.clone();

        store.create_session(&session).await.unwrap();

        let loaded = store.get_session(&session_id).await.unwrap().unwrap();

        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.name, Some("Test Session".to_string()));
    }

    #[tokio::test]
    async fn test_update_session() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();

        let mut session = create_test_session("Original").await;
        let session_id = session.session_id.clone();

        store.create_session(&session).await.unwrap();

        session.name = Some("Updated".to_string());
        store.update_session(&session).await.unwrap();

        let loaded = store.get_session(&session_id).await.unwrap().unwrap();

        assert_eq!(loaded.name, Some("Updated".to_string()));
    }

    #[tokio::test]
    async fn test_delete_session() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();

        let session = create_test_session("To Delete").await;
        let session_id = session.session_id.clone();

        store.create_session(&session).await.unwrap();
        assert!(store.get_session(&session_id).await.unwrap().is_some());

        store.delete_session(&session_id).await.unwrap();
        assert!(store.get_session(&session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();

        let session1 = create_test_session("Session 1").await;
        let session2 = create_test_session("Session 2").await;

        store.create_session(&session1).await.unwrap();
        store.create_session(&session2).await.unwrap();

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.name == "Session 1"));
        assert!(sessions.iter().any(|s| s.name == "Session 2"));
    }

    #[tokio::test]
    async fn test_get_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();

        let result = store.get_session("nonexistent").await.unwrap();
        assert!(result.is_none());
    }
}
