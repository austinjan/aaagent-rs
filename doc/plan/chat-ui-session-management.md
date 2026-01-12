# Chat UI Session Management Plan

- Feature name: `chat-ui-session-management`
- Status: Draft
- Created: 2026-01-08
- Parent plan: [chat-ui-plan.md](./chat-ui-plan.md)

## 1) Overview

### Goal
Implement session lifecycle management to create, load, persist, and manage conversation sessions.

### Scope (In)
- Session CRUD operations (Create, Read, Update, Delete)
- Storage abstraction with in-memory and persistent backends
- Session metadata management (name, tags, timestamps)
- Integration with config system (resolved_config in metadata)
- Session listing and filtering
- Data directory structure (`data/` for all persistent data)

### Non-goals (Out)
- Multi-user session sharing
- Session branching UI (covered by tree visualization)
- Session export/import (future feature)
- Session templates (future feature)

## 1.1) Data Directory Structure

All persistent data files are stored in the `data/` directory:

```
project_root/
├── config.yaml              # Configuration (working directory)
├── secrets.yaml             # API keys (working directory, gitignored)
├── .env                     # Environment variables (working directory, gitignored)
└── data/                    # All persistent data
    ├── sessions/            # Session files
    │   ├── 01HQZC7X9M8K5J3N2P1R4S6T8V.json
    │   ├── 01HQZD8Y0N9L6K4O3Q2S5U7W9X.json
    │   └── ...
    ├── nodes/               # Tree node storage (optional, for FileStore)
    │   ├── node_abc123.json
    │   ├── node_def456.json
    │   └── ...
    ├── audit/               # Audit logs (future)
    │   └── 2026-01-08.log
    └── backups/             # Session backups (future)
        └── 2026-01-08/
```

**Rationale**:
- **`config.yaml` in root**: User-facing configuration, easy to find and edit
- **`data/` for everything else**: Clear separation of code vs. data
- **`data/sessions/`**: One JSON file per session
- **`data/nodes/`**: Optional node storage for file-based persistence
- **`data/audit/`**: Future feature for audit logging
- **`data/backups/`**: Future feature for automated backups

**`.gitignore` updates**:
```gitignore
# Data directory - never commit user data
/data/

# Sensitive config
secrets.yaml
.env
.env.local
.env.*.local
keys/
*.key
```

## 2) Requirements

### Functional Requirements
- [ ] Create new session with initial config
- [ ] Load existing session by ID
- [ ] Save session state (tree structure + metadata)
- [ ] Delete/archive sessions
- [ ] List sessions with filtering (by tag, date, etc.)
- [ ] Update session metadata (name, tags)
- [ ] Prevent system_prompt changes on existing sessions

### Non-functional Requirements
- **Performance**: Session load <50ms for in-memory, <200ms for persistent
- **Reliability**: Atomic saves (no partial session corruption)
- **Scalability**: Support 1000+ sessions per user (persistent storage)
- **Compatibility**: Works with existing `Session` struct

## 3) Design

### Architecture

```
┌─────────────────────────────────────────┐
│         API Layer (axum)                │
│  POST /api/sessions                     │
│  GET /api/sessions/:id                  │
│  PATCH /api/sessions/:id                │
│  DELETE /api/sessions/:id               │
│  GET /api/sessions (list)               │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│       SessionManager                    │
│  - create(config) -> Session            │
│  - load(id) -> Session                  │
│  - save(session)                        │
│  - delete(id)                           │
│  - list(filter) -> Vec<SessionInfo>    │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│       SessionStore (trait)              │
│  MemoryStore  │  FileStore  │  DbStore  │
└─────────────────────────────────────────┘
```

### Session Data Model

**Existing `Session` struct** (from `src/history/session.rs`):
```rust
pub struct Session {
    pub session_id: SessionId,
    pub created_at: i64,
    pub updated_at: i64,
    pub root_node_id: NodeId,
    pub active_leaf_id: NodeId,
    pub head_checkpoint_id: Option<NodeId>,
    
    // Metadata
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub config: SessionConfig,       // Session-level config (system_prompt, etc.)
    pub stats: SessionStats,
    pub checkpoints: HashMap<NodeId, CheckpointData>,
    pub pinned_leaves: Option<Vec<NodeId>>,
    pub archived_tool_results: Option<HashMap<String, ArchivedToolResult>>,
    pub metadata: Option<serde_json::Value>,  // Stores resolved_config
    
    #[serde(skip)]
    store: Option<Arc<dyn TreeStore>>,  // Node storage
}
```

**Extension - Session Metadata for Listing**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: SessionId,
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
    pub last_message_preview: Option<String>,  // First 100 chars of last user message
    pub resolved_config: Option<ResolvedConfig>,  // From session.metadata
}
```

### SessionManager API

```rust
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    node_store: Arc<dyn TreeStore>,  // For nodes within sessions
}

impl SessionManager {
    pub fn new(store: Arc<dyn SessionStore>, node_store: Arc<dyn TreeStore>) -> Self {
        Self { store, node_store }
    }
    
    /// Create a new session with resolved config
    pub async fn create(&self, resolved_config: ResolvedConfig) -> Result<Session> {
        let session_id = new_session_id();
        let root_node_id = new_node_id();
        let now = now();
        
        // Create session with config
        let mut session = Session {
            session_id: session_id.clone(),
            created_at: now,
            updated_at: now,
            root_node_id: root_node_id.clone(),
            active_leaf_id: root_node_id.clone(),
            head_checkpoint_id: None,
            name: None,
            tags: vec![],
            config: SessionConfig {
                system_prompt: Some(resolved_config.session.system_prompt.clone()),
                max_context_tokens: resolved_config.session.max_context_tokens,
                prune_ephemeral_after_days: 7,
                optimization: ContextOptimizationConfig::default(),
            },
            stats: SessionStats::default(),
            checkpoints: HashMap::new(),
            pinned_leaves: None,
            archived_tool_results: None,
            metadata: Some(json!({ "resolved_config": resolved_config })),
            store: Some(self.node_store.clone()),
        };
        
        // Create root node
        let root_node = Node {
            node_id: root_node_id,
            kind: NodeKind::Root,
            parent_id: None,
            children: vec![],
            created_at: now,
            flags: NodeFlags::default(),
            content: None,
            metadata: None,
        };
        
        self.node_store.save_node(&root_node).await?;
        self.store.save_session(&session).await?;
        
        Ok(session)
    }
    
    /// Load an existing session
    pub async fn load(&self, session_id: &str) -> Result<Session> {
        let mut session = self.store.load_session(session_id).await?;
        session.store = Some(self.node_store.clone());
        Ok(session)
    }
    
    /// Save session state (updates updated_at automatically)
    pub async fn save(&self, session: &mut Session) -> Result<()> {
        session.updated_at = now();
        self.store.save_session(session).await
    }
    
    /// Update session metadata only (name, tags)
    pub async fn update_metadata(
        &self,
        session_id: &str,
        name: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<()> {
        let mut session = self.load(session_id).await?;
        
        if let Some(n) = name {
            session.name = Some(n);
        }
        if let Some(t) = tags {
            session.tags = t;
        }
        
        self.save(&mut session).await
    }
    
    /// Delete a session and all its nodes
    pub async fn delete(&self, session_id: &str) -> Result<()> {
        // Load session to get node IDs
        let session = self.load(session_id).await?;
        
        // Delete all nodes in the tree
        let node_ids = self.collect_all_node_ids(&session).await?;
        for node_id in node_ids {
            self.node_store.delete_node(&node_id).await?;
        }
        
        // Delete session
        self.store.delete_session(session_id).await
    }
    
    /// List sessions with optional filtering
    pub async fn list(&self, filter: SessionFilter) -> Result<Vec<SessionMetadata>> {
        self.store.list_sessions(filter).await
    }
    
    /// Get resolved config from session metadata
    pub fn get_resolved_config(&self, session: &Session) -> Result<ResolvedConfig> {
        session.metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| anyhow!("resolved_config not found in session metadata"))
    }
    
    /// Validate config change request (prevent system_prompt changes)
    pub fn validate_config_update(
        &self,
        session: &Session,
        new_config: &ChatConfig,
    ) -> Result<()> {
        if new_config.system_prompt.is_some() {
            bail!("system_prompt is immutable. Create a new session to use a different prompt.");
        }
        Ok(())
    }
    
    // Helper to collect all node IDs in a session tree
    async fn collect_all_node_ids(&self, session: &Session) -> Result<Vec<NodeId>> {
        let mut node_ids = vec![session.root_node_id.clone()];
        let mut to_visit = vec![session.root_node_id.clone()];
        
        while let Some(node_id) = to_visit.pop() {
            let node = self.node_store.load_node(&node_id).await?;
            for child_id in &node.children {
                node_ids.push(child_id.clone());
                to_visit.push(child_id.clone());
            }
        }
        
        Ok(node_ids)
    }
}
```

### SessionStore Trait

```rust
use async_trait::async_trait;

#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Save a session (upsert)
    async fn save_session(&self, session: &Session) -> Result<()>;
    
    /// Load a session by ID
    async fn load_session(&self, session_id: &str) -> Result<Session>;
    
    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    
    /// List sessions with filtering
    async fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<SessionMetadata>>;
    
    /// Check if session exists
    async fn exists(&self, session_id: &str) -> Result<bool>;
}

#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub tags: Option<Vec<String>>,           // Filter by tags (any match)
    pub created_after: Option<i64>,          // Timestamp filter
    pub created_before: Option<i64>,
    pub name_contains: Option<String>,       // Substring search in name
    pub limit: Option<usize>,                // Max results (default: 100)
    pub offset: Option<usize>,               // Pagination offset
    pub sort_by: SessionSortField,           // Sort field
    pub sort_order: SortOrder,               // Ascending/descending
}

#[derive(Debug, Clone, Copy)]
pub enum SessionSortField {
    CreatedAt,
    UpdatedAt,
    Name,
    MessageCount,
}

impl Default for SessionSortField {
    fn default() -> Self {
        Self::UpdatedAt  // Most recent first
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Descending
    }
}
```

### MemoryStore Implementation

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct MemorySessionStore {
    sessions: RwLock<HashMap<SessionId, Session>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn save_session(&self, session: &Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.session_id.clone(), session.clone());
        Ok(())
    }
    
    async fn load_session(&self, session_id: &str) -> Result<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id)
            .cloned()
            .ok_or_else(|| anyhow!("Session '{}' not found", session_id))
    }
    
    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id)
            .ok_or_else(|| anyhow!("Session '{}' not found", session_id))?;
        Ok(())
    }
    
    async fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<SessionMetadata>> {
        let sessions = self.sessions.read().await;
        let mut results: Vec<SessionMetadata> = sessions.values()
            .filter_map(|s| self.to_metadata(s, &filter))
            .collect();
        
        // Sort
        results.sort_by(|a, b| match filter.sort_by {
            SessionSortField::CreatedAt => compare_i64(a.created_at, b.created_at, filter.sort_order),
            SessionSortField::UpdatedAt => compare_i64(a.updated_at, b.updated_at, filter.sort_order),
            SessionSortField::Name => compare_option_str(&a.name, &b.name, filter.sort_order),
            SessionSortField::MessageCount => compare_usize(a.message_count, b.message_count, filter.sort_order),
        });
        
        // Pagination
        let offset = filter.offset.unwrap_or(0);
        let limit = filter.limit.unwrap_or(100);
        Ok(results.into_iter().skip(offset).take(limit).collect())
    }
    
    async fn exists(&self, session_id: &str) -> Result<bool> {
        let sessions = self.sessions.read().await;
        Ok(sessions.contains_key(session_id))
    }
}

impl MemorySessionStore {
    fn to_metadata(&self, session: &Session, filter: &SessionFilter) -> Option<SessionMetadata> {
        // Apply filters
        if let Some(tags) = &filter.tags {
            if !tags.iter().any(|t| session.tags.contains(t)) {
                return None;
            }
        }
        
        if let Some(after) = filter.created_after {
            if session.created_at < after {
                return None;
            }
        }
        
        if let Some(before) = filter.created_before {
            if session.created_at > before {
                return None;
            }
        }
        
        if let Some(name_filter) = &filter.name_contains {
            if let Some(name) = &session.name {
                if !name.to_lowercase().contains(&name_filter.to_lowercase()) {
                    return None;
                }
            } else {
                return None;
            }
        }
        
        // Extract resolved_config
        let resolved_config = session.metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        
        Some(SessionMetadata {
            session_id: session.session_id.clone(),
            name: session.name.clone(),
            tags: session.tags.clone(),
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: session.stats.total_messages,
            last_message_preview: None,  // TODO: Extract from last node
            resolved_config,
        })
    }
}

fn compare_i64(a: i64, b: i64, order: SortOrder) -> std::cmp::Ordering {
    match order {
        SortOrder::Ascending => a.cmp(&b),
        SortOrder::Descending => b.cmp(&a),
    }
}

fn compare_usize(a: usize, b: usize, order: SortOrder) -> std::cmp::Ordering {
    match order {
        SortOrder::Ascending => a.cmp(&b),
        SortOrder::Descending => b.cmp(&a),
    }
}

fn compare_option_str(a: &Option<String>, b: &Option<String>, order: SortOrder) -> std::cmp::Ordering {
    match order {
        SortOrder::Ascending => a.cmp(b),
        SortOrder::Descending => b.cmp(a),
    }
}
```

### FileStore Implementation (Optional)

```rust
use std::path::PathBuf;
use tokio::fs;

pub struct FileSessionStore {
    base_path: PathBuf,  // data/sessions/
}

impl FileSessionStore {
    pub fn new(base_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }
    
    /// Default path: ./data/sessions/
    pub fn default() -> Result<Self> {
        Self::new(PathBuf::from("data/sessions"))
    }
    
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.base_path.join(format!("{}.json", session_id))
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn save_session(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.session_id);
        let json = serde_json::to_string_pretty(session)?;
        fs::write(path, json).await?;
        Ok(())
    }
    
    async fn load_session(&self, session_id: &str) -> Result<Session> {
        let path = self.session_path(session_id);
        let json = fs::read_to_string(path).await?;
        let session: Session = serde_json::from_str(&json)?;
        Ok(session)
    }
    
    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let path = self.session_path(session_id);
        fs::remove_file(path).await?;
        Ok(())
    }
    
    async fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<SessionMetadata>> {
        let mut entries = fs::read_dir(&self.base_path).await?;
        let mut results = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            if let Some(ext) = entry.path().extension() {
                if ext == "json" {
                    if let Ok(json) = fs::read_to_string(entry.path()).await {
                        if let Ok(session) = serde_json::from_str::<Session>(&json) {
                            // Apply same filtering logic as MemoryStore
                            if let Some(metadata) = self.to_metadata(&session, &filter) {
                                results.push(metadata);
                            }
                        }
                    }
                }
            }
        }
        
        // Sort and paginate (same as MemoryStore)
        results.sort_by(|a, b| match filter.sort_by {
            SessionSortField::UpdatedAt => b.updated_at.cmp(&a.updated_at),
            _ => std::cmp::Ordering::Equal,
        });
        
        let offset = filter.offset.unwrap_or(0);
        let limit = filter.limit.unwrap_or(100);
        Ok(results.into_iter().skip(offset).take(limit).collect())
    }
    
    async fn exists(&self, session_id: &str) -> Result<bool> {
        Ok(self.session_path(session_id).exists())
    }
}
```

## 4) API Endpoints

### POST /api/sessions - Create New Session

**Request:**
```json
{
  "config": {
    "preset": "general",
    "system_prompt": "You are a helpful assistant.",
    "intent": {
      "creativity": 0.5,
      "verbosity": "normal",
      "rounds": 30
    }
  },
  "name": "My Research Session",
  "tags": ["research", "important"]
}
```

**Response:**
```json
{
  "session_id": "01HQZC7X9M8K5J3N2P1R4S6T8V",
  "created_at": 1704672000000,
  "resolved_config": {
    "provider": { "model": "gpt-5-mini", ... },
    "agent": { "max_rounds": 30, ... },
    "session": { "system_prompt": "...", ... }
  }
}
```

**Implementation:**
```rust
#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    config: ChatConfig,
    name: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    session_id: SessionId,
    created_at: i64,
    resolved_config: ResolvedConfig,
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, ApiError> {
    // Resolve config
    let resolved = state.config_resolver.resolve(&req.config)?;
    
    // Create session
    let mut session = state.session_manager.create(resolved.clone()).await?;
    
    // Set metadata
    session.name = req.name;
    session.tags = req.tags.unwrap_or_default();
    
    // Save
    state.session_manager.save(&mut session).await?;
    
    Ok(Json(CreateSessionResponse {
        session_id: session.session_id,
        created_at: session.created_at,
        resolved_config: resolved,
    }))
}
```

### GET /api/sessions/:id - Load Session

**Response:**
```json
{
  "session_id": "01HQZC7X9M8K5J3N2P1R4S6T8V",
  "name": "My Research Session",
  "tags": ["research", "important"],
  "created_at": 1704672000000,
  "updated_at": 1704675600000,
  "message_count": 42,
  "resolved_config": { ... }
}
```

**Implementation:**
```rust
async fn get_session(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<SessionMetadata>, ApiError> {
    let session = state.session_manager.load(&session_id).await?;
    let resolved_config = state.session_manager.get_resolved_config(&session)?;
    
    Ok(Json(SessionMetadata {
        session_id: session.session_id,
        name: session.name,
        tags: session.tags,
        created_at: session.created_at,
        updated_at: session.updated_at,
        message_count: session.stats.total_messages,
        last_message_preview: None,
        resolved_config: Some(resolved_config),
    }))
}
```

### PATCH /api/sessions/:id - Update Metadata

**Request:**
```json
{
  "name": "Updated Session Name",
  "tags": ["research", "archived"]
}
```

**Response:** 204 No Content

**Implementation:**
```rust
#[derive(Debug, Deserialize)]
struct UpdateSessionRequest {
    name: Option<String>,
    tags: Option<Vec<String>>,
}

async fn update_session(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<StatusCode, ApiError> {
    state.session_manager.update_metadata(
        &session_id,
        req.name,
        req.tags,
    ).await?;
    
    Ok(StatusCode::NO_CONTENT)
}
```

### DELETE /api/sessions/:id - Delete Session

**Response:** 204 No Content

**Implementation:**
```rust
async fn delete_session(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state.session_manager.delete(&session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

### GET /api/sessions - List Sessions

**Query Parameters:**
- `tags`: Comma-separated tags (filter by any match)
- `created_after`: Unix timestamp (ms)
- `created_before`: Unix timestamp (ms)
- `name`: Substring search in name
- `limit`: Max results (default: 100, max: 1000)
- `offset`: Pagination offset (default: 0)
- `sort_by`: created_at | updated_at | name | message_count
- `sort_order`: asc | desc (default: desc)

**Example:**
```
GET /api/sessions?tags=research,important&limit=20&sort_by=updated_at&sort_order=desc
```

**Response:**
```json
{
  "sessions": [
    {
      "session_id": "01HQZC7X9M8K5J3N2P1R4S6T8V",
      "name": "My Research Session",
      "tags": ["research", "important"],
      "created_at": 1704672000000,
      "updated_at": 1704675600000,
      "message_count": 42,
      "last_message_preview": "Can you explain quantum entanglement?"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Implementation:**
```rust
#[derive(Debug, Deserialize)]
struct ListSessionsQuery {
    tags: Option<String>,           // Comma-separated
    created_after: Option<i64>,
    created_before: Option<i64>,
    name: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListSessionsResponse {
    sessions: Vec<SessionMetadata>,
    total: usize,
    limit: usize,
    offset: usize,
}

async fn list_sessions(
    Query(query): Query<ListSessionsQuery>,
    State(state): State<AppState>,
) -> Result<Json<ListSessionsResponse>, ApiError> {
    let filter = SessionFilter {
        tags: query.tags.map(|s| s.split(',').map(|t| t.trim().to_string()).collect()),
        created_after: query.created_after,
        created_before: query.created_before,
        name_contains: query.name,
        limit: query.limit,
        offset: query.offset,
        sort_by: parse_sort_field(&query.sort_by),
        sort_order: parse_sort_order(&query.sort_order),
    };
    
    let sessions = state.session_manager.list(filter.clone()).await?;
    let total = sessions.len();
    
    Ok(Json(ListSessionsResponse {
        sessions,
        total,
        limit: filter.limit.unwrap_or(100),
        offset: filter.offset.unwrap_or(0),
    }))
}

fn parse_sort_field(s: &Option<String>) -> SessionSortField {
    match s.as_ref().map(|s| s.as_str()) {
        Some("created_at") => SessionSortField::CreatedAt,
        Some("updated_at") => SessionSortField::UpdatedAt,
        Some("name") => SessionSortField::Name,
        Some("message_count") => SessionSortField::MessageCount,
        _ => SessionSortField::default(),
    }
}

fn parse_sort_order(s: &Option<String>) -> SortOrder {
    match s.as_ref().map(|s| s.as_str()) {
        Some("asc") => SortOrder::Ascending,
        Some("desc") => SortOrder::Descending,
        _ => SortOrder::default(),
    }
}
```

## 5) Integration with Chat Endpoint

### Updated Chat Endpoint Flow

```rust
async fn chat(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, ApiError> {
    // 1. Load or create session
    let session = if session_id == "new" {
        // ERROR: Use POST /api/sessions to create new sessions
        return Err(ApiError::BadRequest(
            "Use POST /api/sessions to create new sessions".to_string()
        ));
    } else {
        // Load existing session
        state.session_manager.load(&session_id).await?
    };
    
    // 2. Get resolved config
    let resolved_config = if let Some(cfg) = &req.config {
        // Validate: cannot change system_prompt
        state.session_manager.validate_config_update(&session, cfg)?;
        
        // Resolve new config (updates runtime params only)
        state.config_resolver.resolve(cfg)?
    } else {
        // Use existing config from session
        state.session_manager.get_resolved_config(&session)?
    };
    
    // 3. Create provider based on resolved config
    let provider = create_provider(&resolved_config.provider, &state.config_manager).await?;
    
    // 4. Create agent
    let agent = Agent::with_config(
        session,
        provider,
        state.tools.clone(),
        AgentConfig {
            max_rounds: resolved_config.agent.max_rounds as usize,
            loop_detection: Some(LoopDetectorConfig::default()),
        },
    );
    
    // 5. Start streaming chat in background
    let stream_id = start_chat_stream(agent, req.message, state.session_manager.clone()).await?;
    
    Ok(Json(ChatResponse {
        stream_id,
        resolved_config,
    }))
}
```

### Provider Factory

```rust
async fn create_provider(
    provider_config: &ProviderConfig,
    config_manager: &ConfigManager,
) -> Result<Box<dyn LLMProvider>> {
    let provider_name = get_provider_for_model(&provider_config.model);
    let api_key = config_manager.get_api_key(provider_name)?;
    
    let provider: Box<dyn LLMProvider> = match provider_name {
        "openai" => Box::new(OpenAIProvider::new(
            api_key,
            provider_config.model.clone(),
            provider_config.temperature,
            provider_config.max_tokens,
        )?),
        "anthropic" => Box::new(AnthropicProvider::new(
            api_key,
            provider_config.model.clone(),
            provider_config.temperature,
            provider_config.max_tokens,
        )?),
        "google" => Box::new(GeminiProvider::new(
            api_key,
            provider_config.model.clone(),
            provider_config.temperature,
            provider_config.max_tokens,
        )?),
        _ => bail!("Unknown provider: {}", provider_name),
    };
    
    Ok(provider)
}
```

## 6) AppState Updates

```rust
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub config_manager: Arc<ConfigManager>,
    pub session_manager: Arc<SessionManager>,
    pub tools: Arc<ToolRegistry>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let config_manager = Arc::new(ConfigManager::new()?);
        let config_resolver = Arc::new(ConfigResolver::new()?);
        
        // Initialize storage backends
        // For production: use FileStore with data/ directory
        // For testing: use MemoryStore
        let use_persistent = std::env::var("USE_MEMORY_STORE").is_err();
        
        let (session_store, node_store): (Arc<dyn SessionStore>, Arc<dyn TreeStore>) = if use_persistent {
            // File-based storage in data/ directory
            let session_store = Arc::new(FileSessionStore::new(PathBuf::from("data/sessions"))?);
            let node_store = Arc::new(FileTreeStore::new(PathBuf::from("data/nodes"))?);
            (session_store, node_store)
        } else {
            // In-memory storage
            let session_store = Arc::new(MemorySessionStore::new());
            let node_store = Arc::new(MemoryStore::new());
            (session_store, node_store)
        };
        
        let session_manager = Arc::new(SessionManager::new(session_store, node_store));
        
        // Initialize tools
        let tools = Arc::new(ToolRegistry::new());
        
        Ok(Self {
            config_resolver,
            config_manager,
            session_manager,
            tools,
        })
    }
}
```

## 7) Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_session() {
        let node_store = Arc::new(MemoryStore::new());
        let session_store = Arc::new(MemorySessionStore::new());
        let manager = SessionManager::new(session_store, node_store);
        
        let resolved = test_resolved_config();
        let session = manager.create(resolved).await.unwrap();
        
        assert!(!session.session_id.is_empty());
        assert_eq!(session.stats.total_messages, 0);
    }
    
    #[tokio::test]
    async fn test_load_save_session() {
        let manager = test_session_manager();
        let resolved = test_resolved_config();
        
        let mut session = manager.create(resolved).await.unwrap();
        let session_id = session.session_id.clone();
        
        // Modify and save
        session.name = Some("Test Session".to_string());
        manager.save(&mut session).await.unwrap();
        
        // Load and verify
        let loaded = manager.load(&session_id).await.unwrap();
        assert_eq!(loaded.name, Some("Test Session".to_string()));
    }
    
    #[tokio::test]
    async fn test_delete_session() {
        let manager = test_session_manager();
        let resolved = test_resolved_config();
        
        let session = manager.create(resolved).await.unwrap();
        let session_id = session.session_id.clone();
        
        // Delete
        manager.delete(&session_id).await.unwrap();
        
        // Verify deletion
        assert!(manager.load(&session_id).await.is_err());
    }
    
    #[tokio::test]
    async fn test_list_sessions_with_filter() {
        let manager = test_session_manager();
        
        // Create sessions with tags
        let mut s1 = manager.create(test_resolved_config()).await.unwrap();
        s1.tags = vec!["research".to_string()];
        manager.save(&mut s1).await.unwrap();
        
        let mut s2 = manager.create(test_resolved_config()).await.unwrap();
        s2.tags = vec!["coding".to_string()];
        manager.save(&mut s2).await.unwrap();
        
        // Filter by tag
        let filter = SessionFilter {
            tags: Some(vec!["research".to_string()]),
            ..Default::default()
        };
        
        let results = manager.list(filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tags, vec!["research".to_string()]);
    }
    
    #[tokio::test]
    async fn test_validate_system_prompt_immutable() {
        let manager = test_session_manager();
        let resolved = test_resolved_config();
        let session = manager.create(resolved).await.unwrap();
        
        let mut new_config = ChatConfig::default();
        new_config.system_prompt = Some("New prompt".to_string());
        
        let result = manager.validate_config_update(&session, &new_config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("immutable"));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_session_lifecycle() {
    let state = AppState::new().unwrap();
    
    // 1. Create session
    let create_req = CreateSessionRequest {
        config: ChatConfig {
            preset: "general".to_string(),
            system_prompt: Some("Test prompt".to_string()),
            tools_enabled: true,
            intent: Default::default(),
            overrides: None,
        },
        name: Some("Integration Test".to_string()),
        tags: Some(vec!["test".to_string()]),
    };
    
    let create_resp = create_session(State(state.clone()), Json(create_req))
        .await
        .unwrap();
    let session_id = create_resp.0.session_id;
    
    // 2. Load session
    let session = state.session_manager.load(&session_id).await.unwrap();
    assert_eq!(session.name, Some("Integration Test".to_string()));
    
    // 3. Update metadata
    state.session_manager.update_metadata(
        &session_id,
        Some("Updated Name".to_string()),
        None,
    ).await.unwrap();
    
    // 4. List sessions
    let filter = SessionFilter::default();
    let sessions = state.session_manager.list(filter).await.unwrap();
    assert!(sessions.iter().any(|s| s.session_id == session_id));
    
    // 5. Delete session
    state.session_manager.delete(&session_id).await.unwrap();
    assert!(state.session_manager.load(&session_id).await.is_err());
}
```

## 8) Acceptance Criteria

- [ ] SessionManager creates sessions with resolved_config
- [ ] Sessions persist and load correctly
- [ ] Session metadata (name, tags) can be updated
- [ ] Sessions can be deleted (including all nodes)
- [ ] Session listing supports filtering and pagination
- [ ] system_prompt validation prevents updates on existing sessions
- [ ] MemoryStore implementation works for all operations
- [ ] FileStore implementation works for all operations (optional)
- [ ] API endpoints handle all CRUD operations
- [ ] Chat endpoint integrates with session management
- [ ] Provider factory creates correct provider from config
- [ ] All unit tests pass
- [ ] Integration tests cover full lifecycle

## 9) Data Directory Management

### Initialization

```rust
use std::fs;
use std::path::Path;

pub fn ensure_data_directories() -> Result<()> {
    let dirs = [
        "data/sessions",
        "data/nodes",
        "data/audit",
        "data/backups",
    ];
    
    for dir in &dirs {
        fs::create_dir_all(dir)?;
        
        // Create .gitkeep to preserve directory structure
        let gitkeep = Path::new(dir).join(".gitkeep");
        if !gitkeep.exists() {
            fs::write(gitkeep, "")?;
        }
    }
    
    Ok(())
}
```

**Called from `main()` or `AppState::new()`**:
```rust
fn main() -> anyhow::Result<()> {
    // Ensure data directories exist
    ensure_data_directories()?;
    
    // Initialize app state
    let state = AppState::new()?;
    
    // ... rest of app
}
```

### CLI Commands

```bash
# Initialize data directories
cargo run -- init-data

# Clean up data (with confirmation)
cargo run -- clean-data --sessions --nodes

# Backup data
cargo run -- backup-data --output backup.tar.gz

# List data directory sizes
cargo run -- data-info
```

## 10) Future Enhancements

### Phase 2
- [ ] Session export/import (JSON format to `data/exports/`)
- [ ] Session templates (save config as reusable template)
- [ ] Session search (full-text search in messages)
- [ ] Session analytics (token usage, cost tracking in `data/analytics/`)
- [ ] Audit logging (all operations logged to `data/audit/`)

### Phase 3
- [ ] Database backend (PostgreSQL/SQLite in `data/db/`)
- [ ] Multi-user session sharing
- [ ] Session permissions/access control
- [ ] Session versioning/snapshots (stored in `data/backups/`)
- [ ] Automated cleanup (delete old sessions from `data/sessions/`)

---

## Changelog

- 2026-01-08: Initial draft - session lifecycle management plan
- 2026-01-08: Added data directory structure - all persistent data in `data/`
