# Session Storage Implementation Plan

**Status:** In Progress  
**Created:** 2026-01-12  
**Goal:** Add persistent file-based session storage to replace placeholder data

---

## 1. Architecture

### Storage Backend

```
data/
├── sessions/
│   ├── {session_id}.json       # Session metadata + config
│   └── {session_id}_nodes/     # Tree nodes (optional, for scaling)
│       ├── {node_id}.json
│       └── ...
└── audit/
    └── sessions.log            # Audit trail
```

### Session File Format

```json
{
  "session_id": "01HQXXX...",
  "name": "My Conversation",
  "created_at": 1704672000000,
  "updated_at": 1704758400000,
  "root_node_id": "node_root",
  "active_leaf_id": "node_leaf_123",
  "metadata": {
    "resolved_config": {
      "provider": { "model": "gpt-5-mini", "temperature": 1.0, "max_tokens": 16384 },
      "agent": { "max_rounds": 30, "tools_enabled": true },
      "session": { "system_prompt": "You are...", "max_context_tokens": 200000 }
    },
    "preset": "general",
    "tags": ["work", "research"]
  },
  "stats": {
    "total_nodes": 42,
    "total_tokens_processed": 15000,
    "message_count": 21
  }
}
```

---

## 2. Implementation Steps

### Step 1: Create SessionStore Trait

```rust
// src/storage/mod.rs
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, session: Session) -> Result<()>;
    async fn get_session(&self, id: &str) -> Result<Option<Session>>;
    async fn update_session(&self, session: Session) -> Result<()>;
    async fn delete_session(&self, id: &str) -> Result<()>;
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
}

pub struct SessionSummary {
    pub session_id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: u32,
    pub preset: String,
}
```

### Step 2: Implement FileSessionStore

```rust
// src/storage/file_store.rs
pub struct FileSessionStore {
    base_path: PathBuf,
}

impl FileSessionStore {
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }
    
    fn session_path(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{}.json", id))
    }
}

impl SessionStore for FileSessionStore {
    async fn create_session(&self, session: Session) -> Result<()> {
        let path = self.session_path(&session.session_id);
        let json = serde_json::to_string_pretty(&session)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
    
    async fn get_session(&self, id: &str) -> Result<Option<Session>> {
        let path = self.session_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(path).await?;
        let session = serde_json::from_str(&content)?;
        Ok(Some(session))
    }
    
    // ... other methods
}
```

### Step 3: Add to AppState

```rust
// src/api/mod.rs
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub session_store: Arc<dyn SessionStore>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        // Ensure data directory exists
        fs::create_dir_all("data/sessions")?;
        
        Ok(Self {
            config_resolver: Arc::new(ConfigResolver::new()?),
            session_store: Arc::new(FileSessionStore::new("data/sessions")?),
        })
    }
}
```

### Step 4: Update API Endpoints

```rust
// src/api/mod.rs - sessions module

pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let name = req.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("New Session");
    
    let preset = req.get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("general");
    
    // Create config
    let config = ChatConfig {
        preset: preset.to_string(),
        system_prompt: req.get("system_prompt")
            .and_then(|v| v.as_str())
            .map(String::from),
        tools_enabled: true,
        intent: Default::default(),
        overrides: None,
    };
    
    // Resolve config
    let resolved = state.config_resolver.resolve(&config)?;
    
    // Create session
    let session_id = format!("session-{}", ulid::Ulid::new());
    let now = chrono::Utc::now().timestamp_millis();
    
    let mut session = Session::new(
        Arc::new(MemoryStore::new()),
        SessionConfig {
            system_prompt: Some(resolved.session.system_prompt.clone()),
            max_context_tokens: resolved.session.max_context_tokens,
            ..Default::default()
        }
    ).await?;
    
    // Store resolved config in metadata
    session.metadata = Some(serde_json::json!({
        "resolved_config": resolved,
        "preset": preset,
    }));
    
    // Save to disk
    state.session_store.create_session(session.clone()).await?;
    
    Ok(Json(json!({
        "session_id": session_id,
        "name": name,
        "created_at": now,
        "updated_at": now,
        "resolved_config": resolved,
    })))
}

pub async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let sessions = state.session_store.list_sessions().await?;
    
    Ok(Json(json!({
        "sessions": sessions,
        "total": sessions.len(),
    })))
}

pub async fn get_config(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let session = state.session_store
        .get_session(&session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;
    
    // Extract resolved_config from metadata
    let resolved_config = session.metadata
        .as_ref()
        .and_then(|m| m.get("resolved_config"))
        .and_then(|c| serde_json::from_value(c.clone()).ok())
        .ok_or_else(|| ApiError::Internal("Config not found in session".to_string()))?;
    
    // Derive editable config from resolved
    let editable_config = derive_editable_config(&resolved_config);
    
    Ok(Json(ConfigResponse {
        resolved_config,
        editable_config,
    }))
}
```

---

## 3. File Structure

### Module Organization

```
src/
├── storage/
│   ├── mod.rs              # SessionStore trait, SessionSummary
│   ├── file_store.rs       # FileSessionStore implementation
│   └── memory_store.rs     # In-memory (for tests)
├── api/
│   └── mod.rs              # Updated to use SessionStore
└── lib.rs                  # pub mod storage;
```

---

## 4. Testing Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_create_and_get_session() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();
        
        let session = create_test_session();
        store.create_session(session.clone()).await.unwrap();
        
        let loaded = store.get_session(&session.session_id)
            .await
            .unwrap()
            .unwrap();
        
        assert_eq!(loaded.session_id, session.session_id);
    }
    
    #[tokio::test]
    async fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileSessionStore::new(temp_dir.path()).unwrap();
        
        store.create_session(create_test_session()).await.unwrap();
        store.create_session(create_test_session()).await.unwrap();
        
        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
    }
}
```

### Integration Tests

1. Create session via API → verify file exists in `data/sessions/`
2. Get session via API → verify correct data returned
3. Update config → verify file updated
4. Restart server → verify sessions still available

---

## 5. Timeline

- **Step 1-2:** Create trait + FileStore (2h)
- **Step 3:** Update AppState (1h)
- **Step 4:** Update API endpoints (3h)
- **Testing:** Unit + integration tests (2h)

**Total:** ~8 hours (1 day)

---

## 6. Acceptance Criteria

- [ ] FileSessionStore implements SessionStore trait
- [ ] Sessions persist to `data/sessions/{id}.json`
- [ ] API endpoints use real storage (no more placeholders)
- [ ] Create session → saves config to metadata
- [ ] Get config → loads from real session file
- [ ] Update config → persists changes
- [ ] List sessions → reads from disk
- [ ] Server restart → sessions still available
- [ ] Unit tests pass (>90% coverage)
- [ ] Integration tests verify end-to-end flow

---

## 7. Future Enhancements

- [ ] Session search/filtering
- [ ] Batch operations (archive, delete multiple)
- [ ] Session export/import
- [ ] SQLite backend (for larger scale)
- [ ] Session compression (for old sessions)

---

**Ready to implement!** Starting with Step 1.
