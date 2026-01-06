# Tree Message Model Implementation Plan

## Feature Name
Tree-Based Conversation History System

## Status
🚧 **IN PROGRESS** - Phase 1-6 Complete (6/10 phases, Phase 4 integrated into 2)

## Priority
🔴 **HIGH** - Core infrastructure for agent memory and branching

---

## Objective

Implement a tree-based conversation history system that:
- Stores conversation + tool execution as a tree structure
- Supports branching and replay from any point
- Uses checkpoints for context compaction (similar to git commits)
- Supports safe pruning of old history without breaking active branches
- **Keeps LLM providers linear** (they only see `Vec<Message>`)

---

## Background

The current `aaagent-rs` implementation stores conversation history as a linear `Vec<Message>` within each provider. This limits:
- **No branching**: Cannot explore alternative conversation paths
- **No time travel**: Cannot retry from a previous point
- **Simple pruning**: Only keeps last N tool turns, no intelligent compaction
- **No context reuse**: Cannot create checkpoints for long conversations

A tree-based system enables:
- Multiple conversation branches from any point
- Checkpoint-based context compaction (like git commits)
- Safe pruning of old history while preserving active paths
- Better support for long-running conversations

---

## Architecture

### Layer Separation

```
┌─────────────────────────────────────┐
│  Application Layer                  │
│  - CLI, UI                          │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│  Agent Layer (NEW)                  │
│  - Orchestrates Session + Provider  │
│  - Auto checkpoint                  │
│  - Tool execution                   │
└──────┬───────────────────┬──────────┘
       │                   │
┌──────▼────────┐   ┌──────▼─────────┐
│ Session       │   │ LLMProvider    │
│ (Tree)        │   │ (Linear)       │
│               │   │                │
│ get_context() │   │ chat_loop(     │
│ → Vec<Message>│   │   Vec<Message> │
│               │   │ )              │
└──────┬────────┘   └────────────────┘
       │
┌──────▼──────────────────────────────┐
│  TreeStore (Storage Backend)        │
│  - Memory / SQLite / JSONL          │
└─────────────────────────────────────┘
```

### Key Principle

**LLM Providers remain stateless and linear**
- Providers accept `Vec<Message>` (extracted from tree)
- Providers don't know about tree structure
- All history management happens in Session layer
- Compatible with all LLM APIs (OpenAI, Anthropic, Gemini)

---

## Terminology

- **Session**: A conversation workspace that owns a history tree
- **Node**: One history record (message/tool/checkpoint) in the tree
- **Path**: Linear chain from root to leaf via `parent_id`
- **Active leaf**: Current "HEAD" of the session
- **Checkpoint**: Node containing compacted summary for a range of nodes
- **Effective context start**: Nearest checkpoint ancestor of active leaf
- **ULID**: Universally Unique Lexicographically Sortable Identifier
  - Time-ordered: Earlier ULIDs sort before later ones
  - Benefits: `ORDER BY id == ORDER BY created_at`, better B-tree locality, faster range queries

---

## Design Invariants

These are **immutable rules** that MUST be enforced throughout the implementation.

### 1. Append-Only Semantics & Node Immutability
- **Rule**: History is append-only. Nodes are immutable after insertion.
- **Immutable Fields**: All fields except `flags` and `pruned_at` (metadata only)
- **To "change" a node**: Branch from its parent and create a new alternative path
- **Enforcement**: 
  - `TreeStore` has NO `update_node()` method
  - Only narrow APIs: `update_node_flags()`, `mark_node_pruned()`
  - No deletion from active paths (only pruning of orphaned branches)
- **Rationale**: 
  - Prevents history rewriting (like git commits)
  - Enables time-travel and branching
  - Audit trail of all conversation attempts

### 2. Checkpoint as Metadata (Not Node)
- **Rule**: Checkpoints are metadata attached to existing nodes, not separate tree nodes
- **Storage**: `Session::checkpoints: HashMap<NodeId, CheckpointData>`
- **Visual**:
  ```
  Tree structure (unchanged):
  root → A → B → C → D → E (active_leaf)
  
  Checkpoint metadata:
  checkpoints[C] = CheckpointData {
      summary: "Summary of root→C",
      created_at: ...,
      strategy: "auto_count",
  }
  
  Path traversal from E:
  1. Walk: E → D → C
  2. Check: checkpoints.get(C)? → Found!
  3. Stop and use summary instead of C → B → A → root
  ```
- **Enforcement**: Validate checkpoint exists before marking node
- **Rationale**: 
  - Tree structure remains immutable (no node insertion needed)
  - Can add checkpoints retroactively without modifying tree
  - Simpler implementation (just a HashMap lookup)
  - No parent_id complexity

### 3. Head Checkpoint Cache Invalidation
- **Rule**: `Session::head_checkpoint_id` is invalidated when:
  - `branch_from()` is called
  - `switch_to()` is called
  - Set to `Some(checkpoint_id)` when `create_checkpoint()` is called
- **Recomputation**: Lazy on next `get_context()` call
- **Rationale**: Performance optimization without stale cache bugs

### 4. Protected Set for Pruning
- **Rule**: Protected Set = Active Paths ∪ All Checkpoints ∪ Important Nodes ∪ Pinned Leaves
- **Enforcement**: `Session::compute_protected_set()` before any pruning
- **Validation**: `Session::prune_safe()` checks membership before deletion
- **Rationale**: Prevents breaking active conversation branches

### 5. get_path_to_root is Internal Only
- **Rule**: `TreeStore::get_path_to_root_internal()` MUST NOT be called from application code
- **Public API**: Only `Session::get_context()` and `Session::get_context_from()`
- **Enforcement**: Mark method with warning comment, use `pub(crate)` visibility if possible
- **Rationale**: Prevents bypassing checkpoint logic

### 6. Single Root per Session
- **Rule**: Each session has exactly one root node with `parent_id == None`
- **Enforcement**: Validate in `Session::new()` and storage layer
- **Rationale**: Simplifies tree traversal algorithms

### 7. Append-Only JSONL
- **Rule**: `nodes.jsonl` is append-only, no line modifications
- **Updates**: Only flags changes via separate mechanism (rebuild index or metadata file)
- **Enforcement**: `JSONLStore::append_node_to_file()` uses append mode
- **Rationale**: Safe, crash-resistant, easy to recover

**Example: "Changing" a message via branching**:
```
Scenario: User wants to rephrase their question

❌ WRONG (would be mutation):
User: "What's the weather?" (node A)
→ edit node A content to "What's the temperature?"  // FORBIDDEN!

✅ CORRECT (append-only via branching):
root → User: "What's the weather?" (node A) → Assistant: "..." (node B)
       ↓
       User: "What's the temperature?" (node C) → Assistant: "..." (node D)

Timeline:
1. Original path: root → A → B (active_leaf = B)
2. User wants to retry with different question
3. Branch from root: session.branch_from(root_id)
4. Append new message: session.append_message("What's the temperature?") → node C
5. Continue conversation from C → D
6. Now have two conversation branches preserved
```

### 8. Extensibility via Optional Metadata
- **Rule**: New features SHOULD use extension fields instead of new top-level fields
- **Extension Points**:
  - `Node::metadata: Option<serde_json::Value>`
  - `Session::metadata: Option<serde_json::Value>`
  - `CheckpointMetadata::extensions: Option<serde_json::Value>`
- **Core vs Extended**: Core fields are required for basic functionality; extended fields are optional
- **Rationale**: Backward compatibility, flexible evolution, simpler initial implementation

### 9. Tool Sandwich Constraint
- **Rule**: Tool result messages MUST immediately follow their corresponding assistant message with tool_calls
- **Pattern**: `Assistant(tool_calls) → Tool(result)* → Assistant(response)`
- **Enforcement**: Validate in `Session::get_context()` before returning to provider
- **Error Prevention**: Context extraction must preserve tool call/result pairs atomically
- **Rationale**: LLM APIs require strict message ordering, orphaned tool results cause API errors

### 10. No Checkpoint-on-Checkpoint
- **Rule**: Auto-checkpoint logic MUST check if `active_leaf_id` already has a checkpoint
- **Guard**: `if session.checkpoints.contains_key(&active_leaf_id) { return Ok(()); }`
- **Enforcement**: First step in `Agent::auto_checkpoint_if_needed()`
- **Rationale**: Prevents checkpoint duplication, avoids feedback loop with `get_context()`

---

## Requirements

### Functional Requirements

#### 1. Tree Storage
- Store messages as tree nodes with parent-child relationships
- Support multiple children per node (branching)
- Each node has: `node_id`, `parent_id`, `kind`, `content`, metadata
- Node kinds: `Root`, `Message`, `Tool`, `Checkpoint`

#### 2. Session Management
- Each session owns one tree
- Track active leaf (current "HEAD")
- Support multiple sessions
- Session metadata: name, tags, config, stats

#### 3. Context Extraction
- Extract linear `Vec<Message>` from tree
- Walk from active leaf to root (or checkpoint)
- Stop at checkpoint and use summary
- Skip nodes marked as `hidden`
- Reverse to get chronological order

#### 4. Branching
- Create branch from any node
- Switch between branches
- List all active branches
- Find common ancestor of two branches

#### 5. Checkpoints
- Create checkpoint covering a range of nodes
- Store summary, coverage metadata, stats
- Auto-checkpoint based on message count or token limit
- Manual checkpoint on demand
- Checkpoints marked as `important` (never pruned)

#### 6. Pruning (Future)
- Prune orphaned branches (no active leaves)
- Prune ephemeral nodes after time limit
- Safe pruning (never prune ancestors of active leaves)
- Respect `important` and `hidden` flags

#### 7. Replay
- Branch from any historical node
- Continue conversation from that point
- Multiple what-if scenarios

### Non-Functional Requirements

#### 1. Storage Backends
- Pluggable storage via `TreeStore` trait
- In-memory implementation (default)
- SQLite implementation (persistent)
- JSONL implementation (optional, human-readable)

#### 2. Performance
- Context extraction: O(depth) from leaf to checkpoint/root
- Node insertion: O(1)
- Branch listing: O(nodes in session)
- Storage indexes on: `session_id`, `parent_id`, `(parent_id, seq)`

#### 3. Compatibility
- No changes to LLM provider APIs
- Providers remain linear (accept `Vec<Message>`)
- Backward compatible with existing tools/helpers

#### 4. Concurrency
- Thread-safe session operations
- Support concurrent reads
- Single writer per session (optimistic locking)

---

## Technical Design

### 1. Data Models

#### Node Structure

```rust
// src/history/node.rs

/// A node in the conversation tree.
/// 
/// **Immutability**: All fields except `flags` are immutable after insertion.
/// This guarantees append-only semantics and prevents history rewriting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    // Identity (IMMUTABLE)
    pub node_id: NodeId,
    pub session_id: SessionId,
    pub parent_id: Option<NodeId>,  // null only for root
    
    // Type (IMMUTABLE)
    pub kind: NodeKind,
    pub role: Option<Role>,  // nullable, required for Message kind
    
    // Content (IMMUTABLE)
    pub content_type: ContentType,
    pub content: String,
    
    // Metadata (IMMUTABLE except flags)
    pub created_at: i64,  // Unix timestamp
    pub seq: u32,  // ordering among siblings
    pub flags: NodeFlags,  // MUTABLE (only field that can change)
    
    // Kind-specific data (IMMUTABLE)
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    
    // Pruning metadata (set by pruning operations, not part of initial data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruned_at: Option<i64>,
    
    // ============ Future Extension Point ============
    
    /// Arbitrary metadata for future extensions (e.g., embeddings, tags, reactions)
    /// Use this instead of adding new top-level fields to maintain backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

// Note: CheckpointMetadata removed - checkpoints are stored in Session, not in nodes

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Root,
    Message,
    Tool,
    // Note: No Checkpoint kind - checkpoints are metadata, not nodes
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Json,
    Markdown,
    
    // For multimodal content (images, audio, etc.)
    Base64,      // Base64-encoded binary data (e.g., images for vision models)
    
    // Future: specific types for better handling
    // ImageUrl,  // URL to image
    // Binary,    // Raw binary (for local storage)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeFlags {
    pub important: bool,   // Never prune
    pub ephemeral: bool,   // Can prune after time
    pub hidden: bool,      // Skip in context extraction
}

/// Checkpoint data stored as metadata, not as tree nodes.
/// 
/// Checkpoints are attached to existing nodes via `Session::checkpoints` HashMap.
/// The tree structure remains unchanged - checkpoints are just markers.
/// 
/// Visual example:

/// Tree structure (immutable):
/// root → A → B → C → D → E
/// 
/// Checkpoint map:
/// checkpoints[C] = CheckpointData {
///     summary: "Summary of root→C",
///     created_at: 1704412800,
///     strategy: "auto_count",
/// }
/// 
/// Traversal from E:
/// 1. Walk: E → D → C
/// 2. Found checkpoint at C: use summary, stop walking
/// 3. Result: [summary, D, E]

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    // ============ Core Fields (Required) ============
    
    /// Compacted summary of the conversation up to this node
    pub summary: String,
    
    /// When this checkpoint was created
    pub created_at: i64,
    
    // ============ Extended Metadata (Optional) ============
    
    /// Strategy used to create this checkpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,  // "manual" | "auto_count" | "auto_token_limit"
    
    /// Statistics about the checkpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CheckpointStats>,
    
    /// Future extension point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub nodes_covered: u32,
    pub total_tokens: u32,
    pub summary_tokens: u32,
    pub compression_ratio: f32,
    pub covered_time_range: (i64, i64),
}

// Type aliases
pub type NodeId = String;     // ULID (time-ordered, sortable)
pub type SessionId = String;  // ULID (time-ordered, sortable)

// Helper functions
impl NodeId {
    pub fn new() -> Self {
        ulid::Ulid::new().to_string()
    }
    
    pub fn from_timestamp(timestamp_ms: u64) -> Self {
        ulid::Ulid::from_timestamp_ms(timestamp_ms).to_string()
    }
}

impl SessionId {
    pub fn new() -> Self {
        ulid::Ulid::new().to_string()
    }
}
```

#### Session Structure

```rust
// src/history/session.rs

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: SessionId,
    pub created_at: i64,
    pub updated_at: i64,
    pub root_node_id: NodeId,
    pub active_leaf_id: NodeId,
    
    /// Cache of the nearest checkpoint on path from active_leaf to root.
    /// 
    /// **Invalidation Rules**:
    /// - Set to None when: branch_from() or switch_to() is called
    /// - Recomputed lazily on next get_context() call
    /// - Updated when create_checkpoint() is called
    /// 
    /// This is a performance optimization to avoid path traversal on every context extraction.
    pub head_checkpoint_id: Option<NodeId>,  // cache
    
    // Metadata
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub config: SessionConfig,
    pub stats: SessionStats,
    
    /// Checkpoints: node_id -> checkpoint data
    /// Maps nodes to their checkpoint summaries
    pub checkpoints: HashMap<NodeId, CheckpointData>,
    
    /// Optional: pinned leaf nodes to protect from pruning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_leaves: Option<Vec<NodeId>>,
    
    /// Future extension point: arbitrary session metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    
    // Runtime (not serialized)
    #[serde(skip)]
    store: Arc<dyn TreeStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub auto_checkpoint_every: Option<u32>,  // messages
    pub auto_checkpoint_token_limit: Option<u32>,
    pub auto_checkpoint_large_content: Option<u32>,  // tokens per message
    pub max_context_tokens: u32,
    pub prune_ephemeral_after_days: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_checkpoint_every: Some(50),
            auto_checkpoint_token_limit: Some(100_000),
            auto_checkpoint_large_content: Some(5_000),  // checkpoint if single message > 5k tokens
            max_context_tokens: 200_000,
            prune_ephemeral_after_days: 7,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_nodes: u32,
    pub active_branches: u32,
    pub total_checkpoints: u32,
    pub total_tokens_processed: u64,
}
```

### 2. JSONL File Storage Design

#### Directory Structure

```
sessions/
├── {session_id}/
│   ├── session.json          # Session metadata (updated on changes)
│   ├── nodes.jsonl           # Append-only log of all nodes
│   └── index.json            # Optional: node_id → line_number cache
└── sessions.json             # List of all sessions
```

#### File Formats

**session.json** (updated on session changes):
```json
{
  "session_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "created_at": 1704412800,
  "updated_at": 1704499200,
  "root_node_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "active_leaf_id": "01BX5ZZKBKACTAV9WEVGEMMVRY",
  "head_checkpoint_id": "01BX5ZZKBKACTAV9WEVGEMMVRZ",
  "name": "My Conversation",
  "tags": ["debug", "v1"],
  "config": { ... },
  "stats": { ... }
}
```

**Benefits of persisting `head_checkpoint_id`**:
- **Fast session load**: Start from checkpoint, skip tree traversal
- **Performance**: Don't need to scan entire history on startup
- **Optimization**: Cache hit on first `get_context()` call

**nodes.jsonl** (append-only, one node per line):
```jsonl
{"node_id":"root-id","session_id":"session-id","parent_id":null,"kind":"Root",...}
{"node_id":"msg-1","session_id":"session-id","parent_id":"root-id","kind":"Message",...}
{"node_id":"msg-2","session_id":"session-id","parent_id":"msg-1","kind":"Message",...}
```

**index.json** (optional cache, rebuild if corrupted):
```json
{
  "node-id-1": 0,
  "node-id-2": 1,
  "node-id-3": 2
}
```

#### Implementation Strategy

```rust
// src/history/jsonl_store.rs

pub struct JSONLStore {
    base_dir: PathBuf,  // e.g., "./sessions"
    // In-memory cache for fast lookup (loaded on demand)
    cache: Arc<RwLock<HashMap<SessionId, SessionCache>>>,
}

struct SessionCache {
    session: Session,
    node_index: HashMap<NodeId, u64>,  // node_id → line number in JSONL
    nodes: Option<HashMap<NodeId, Node>>,  // Lazy loaded
}

impl JSONLStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self;
    
    // Lazy load: only read session.json initially
    async fn load_session(&self, session_id: SessionId) -> Result<Session>;
    
    // Load specific node from JSONL by line number
    async fn load_node_at_line(&self, session_id: SessionId, line: u64) -> Result<Node>;
    
    // Rebuild index from JSONL (on corruption or first load)
    async fn rebuild_index(&self, session_id: SessionId) -> Result<()>;
    
    // Append node to JSONL (atomic)
    async fn append_node_to_file(&self, session_id: SessionId, node: &Node) -> Result<()>;
}
```

#### Atomic Writes

```rust
async fn append_node_to_file(&self, session_id: SessionId, node: &Node) -> Result<()> {
    let nodes_file = self.get_nodes_path(&session_id);
    
    // 1. Serialize node
    let mut line = serde_json::to_string(node)?;
    line.push('\n');
    
    // 2. Append atomically (OpenOptions::append)
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&nodes_file)
        .await?;
    
    file.write_all(line.as_bytes()).await?;
    file.sync_all().await?;  // Ensure written to disk
    
    Ok(())
}

async fn update_session_file(&self, session: &Session) -> Result<()> {
    let session_file = self.get_session_path(&session.session_id);
    let temp_file = session_file.with_extension("tmp");
    
    // 1. Write to temp file
    let json = serde_json::to_string_pretty(session)?;
    tokio::fs::write(&temp_file, json).await?;
    
    // 2. Atomic rename (atomic on most filesystems)
    tokio::fs::rename(&temp_file, &session_file).await?;
    
    Ok(())
}
```

#### Lazy Loading Strategy

**On Session Load**:
1. Read `session.json` only
2. Read `index.json` (or rebuild if missing)
3. Don't load nodes until needed

**On Node Access**:
1. Check cache first
2. If not cached, use index to find line number
3. Seek to line and parse single JSON object
4. Cache the node

**On Context Extraction**:
1. Read only nodes in the path (from leaf to root)
2. Use index for fast lookup
3. Parse only needed lines

### 3. Storage Interface

```rust
// src/history/storage.rs

/// TreeStore provides append-only storage for conversation tree nodes.
/// 
/// **Immutability Guarantee**: Nodes are immutable after insertion.
/// Core fields (node_id, parent_id, content, etc.) CANNOT be modified.
/// Only metadata (flags, pruning markers) may be updated.
#[async_trait]
pub trait TreeStore: Send + Sync {
    // Node operations (immutable)
    async fn insert_node(&self, node: Node) -> Result<NodeId>;
    async fn get_node(&self, node_id: NodeId) -> Result<Option<Node>>;
    
    // Metadata updates (ONLY these fields allowed):
    async fn update_node_flags(&self, node_id: NodeId, flags: NodeFlags) -> Result<()>;
    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()>;
    
    // Deletion (for pruning only, validates safety)
    async fn delete_node(&self, node_id: NodeId) -> Result<()>;
    
    // Query operations (use via Session, not directly)
    async fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>>;
    
    /// **Internal use only** - Do NOT call directly from application code.
    /// Use `Session::get_context()` instead, which handles checkpoints correctly.
    /// 
    /// This method blindly returns all nodes to root without stopping at checkpoints.
    async fn get_path_to_root_internal(&self, node_id: NodeId) -> Result<Vec<Node>>;
    
    async fn find_nodes(
        &self,
        session_id: SessionId,
        filter: NodeFilter,
    ) -> Result<Vec<Node>>;
    
    // Session operations
    async fn create_session(&self, session: Session) -> Result<SessionId>;
    async fn get_session(&self, session_id: SessionId) -> Result<Option<Session>>;
    async fn update_session(&self, session: Session) -> Result<()>;
    async fn list_sessions(&self) -> Result<Vec<Session>>;
    
    // Batch operations
    async fn get_nodes_batch(&self, node_ids: Vec<NodeId>) -> Result<Vec<Node>>;
    async fn insert_nodes_batch(&self, nodes: Vec<Node>) -> Result<Vec<NodeId>>;
}

#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    pub kinds: Option<Vec<NodeKind>>,
    pub roles: Option<Vec<Role>>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub flags: Option<NodeFlags>,
    pub content_search: Option<String>,
}
```

### 3. Session API

```rust
// src/history/session.rs

impl Session {
    /// Create a new session
    pub async fn new(
        store: Arc<dyn TreeStore>,
        config: SessionConfig,
    ) -> Result<Self>;
    
    /// Extract linear context from active leaf to root/checkpoint
    pub async fn get_context(&self) -> Result<Vec<Message>>;
    
    /// Extract linear context from specific leaf
    pub async fn get_context_from(&self, leaf_id: NodeId) -> Result<Vec<Message>>;
    
    /// Append a message node
    pub async fn append_message(&mut self, msg: Message) -> Result<NodeId>;
    
    /// Create a branch from specific node
    pub async fn branch_from(&mut self, node_id: NodeId) -> Result<NodeId>;
    
    /// Switch to different branch (leaf)
    pub async fn switch_to(&mut self, leaf_id: NodeId) -> Result<()>;
    
    /// Create a checkpoint at a specific node (metadata, not a new node).
    /// 
    /// # Arguments
    /// - `node_id`: The node to mark as a checkpoint
    /// - `summary`: Compacted summary of conversation up to this node
    /// - `strategy`: How the checkpoint was created ("manual", "auto_count", etc.)
    /// 
    /// # Example
    /// ```
    /// // Mark node C as a checkpoint
    /// session.create_checkpoint(
    ///     node_c_id,
    ///     "Summary of conversation so far...".to_string(),
    ///     "auto_count"
    /// ).await?;
    /// 
    /// // Tree structure unchanged:
    /// // root → A → B → C → D → E
    /// 
    /// // Checkpoint map updated:
    /// // session.checkpoints[C] = CheckpointData { summary, ... }
    /// 
    /// // Future get_context() from E will stop at C and use summary
    /// ```
    pub async fn create_checkpoint(
        &mut self,
        node_id: NodeId,
        summary: String,
        strategy: &str,
    ) -> Result<()>;
    
    /// Get all leaf nodes (branch tips)
    /// 
    /// A leaf is any node with no children (except Root).
    /// This includes both active and inactive branches.
    pub async fn get_branches(&self) -> Result<Vec<BranchInfo>>;
    
    /// Get all leaf node IDs in this session
    /// 
    /// Returns ALL leaves, including:
    /// - `active_leaf_id` (current HEAD)
    /// - Inactive branch tips (not actively being used)
    /// - Pinned leaves (explicitly marked for preservation)
    /// 
    /// Used by pruning to determine protected paths.
    pub async fn get_all_leaf_ids(&self) -> Result<Vec<NodeId>>;
    
    /// Get path range between two nodes
    async fn get_path_range(
        &self,
        from_node: NodeId,
        to_node: NodeId,
    ) -> Result<Vec<Node>>;
}
```

**Key Algorithm: Context Extraction**

```rust
pub async fn get_context_from(&self, leaf_id: NodeId) -> Result<Vec<Message>> {
    let mut messages = Vec::new();
    let mut current_id = Some(leaf_id);
    
    // Step 1: Extract messages from tree
    while let Some(node_id) = current_id {
        let node = self.store.get_node(node_id.clone()).await?
            .ok_or_else(|| Error::NodeNotFound(node_id.clone()))?;
        
        // Check if this node has a checkpoint
        if let Some(checkpoint) = self.checkpoints.get(&node_id) {
            // Stop at checkpoint, use summary
            messages.push(Message {
                role: Role::System,
                content: checkpoint.summary.clone(),
                tool_call_id: None,
                tool_calls: None,
            });
            break;
        }
        
        match node.kind {
            NodeKind::Message | NodeKind::Tool => {
                if !node.flags.hidden {
                    messages.push(node.to_message());
                }
            }
            NodeKind::Root => break,
        }
        
        current_id = node.parent_id;
    }
    
    messages.reverse();
    
    // Step 2: Inject dynamic system prompt from config
    if let Some(system_prompt) = &self.config.system_prompt {
        messages.insert(0, Message {
            role: Role::System,
            content: system_prompt.clone(),
            tool_call_id: None,
            tool_calls: None,
        });
    }
    
    // Step 3: Validate Tool Sandwich Constraint
    Self::validate_tool_sandwich(&messages)?;
    
    Ok(messages)
}

/// Validate that tool results immediately follow their corresponding assistant messages.
/// 
/// **Tool Sandwich Pattern**: `Assistant(tool_calls) → Tool(result)* → Assistant(response)`
fn validate_tool_sandwich(messages: &[Message]) -> Result<()> {
    let mut expecting_tool_results = false;
    let mut tool_calls_count = 0;
    
    for (i, msg) in messages.iter().enumerate() {
        match msg.role {
            Role::Assistant => {
                if let Some(tool_calls) = &msg.tool_calls {
                    if !tool_calls.is_empty() {
                        expecting_tool_results = true;
                        tool_calls_count = tool_calls.len();
                    } else {
                        expecting_tool_results = false;
                    }
                } else {
                    expecting_tool_results = false;
                }
            }
            Role::Tool => {
                if !expecting_tool_results {
                    return Err(Error::OrphanedToolResult {
                        position: i,
                        tool_call_id: msg.tool_call_id.clone(),
                    });
                }
                tool_calls_count -= 1;
                if tool_calls_count == 0 {
                    expecting_tool_results = false;
                }
            }
            Role::User => {
                if expecting_tool_results {
                    return Err(Error::IncompletToolSandwich {
                        position: i,
                        missing_results: tool_calls_count,
                    });
                }
            }
            _ => {}
        }
    }
    
    Ok(())
}
```

### 4. Agent Layer

```rust
// src/agent/mod.rs

pub struct Agent {
    session: Session,
    provider: Box<dyn LLMProvider>,
    tools: ToolRegistry,
}

impl Agent {
    pub fn new(
        session: Session,
        provider: Box<dyn LLMProvider>,
        tools: ToolRegistry,
    ) -> Self;
    
    /// Main chat interface
    pub async fn chat(&mut self, user_message: &str) -> Result<String>;
    
    /// Branch from specific node and continue
    pub async fn branch_and_retry(
        &mut self,
        from_node_id: NodeId,
        new_user_message: &str,
    ) -> Result<String>;
    
    /// Manually create checkpoint
    pub async fn checkpoint(&mut self) -> Result<NodeId>;
    
    /// Auto checkpoint if needed (called after each chat)
    async fn auto_checkpoint_if_needed(&mut self) -> Result<()>;
    
    /// Generate summary for checkpoint using LLM
    async fn generate_summary(&self, context: &[Message]) -> Result<String>;
}
```

**Key Flow: Agent.chat()**

```rust
pub async fn chat(&mut self, user_message: &str) -> Result<String> {
    // 1. Add user message to tree
    self.session.append_message(Message {
        role: Role::User,
        content: user_message.to_string(),
        tool_call_id: None,
        tool_calls: None,
    }).await?;
    
    // 2. Extract linear context from tree
    let context = self.session.get_context().await?;
    
    // 3. Call provider with linear history (provider is stateless)
    let tools = self.tools.get_tools_for_llm();
    let mut handle = self.provider.chat_loop(context, Some(tools)).await?;
    
    // 4. Process response loop
    let mut response_content = String::new();
    
    while let Some(event) = handle.next().await {
        match event? {
            LoopStep::Content(text) => {
                response_content.push_str(&text);
            }
            LoopStep::ToolCallsRequested { tool_calls, content } => {
                // Execute tools and add to tree
                let results = self.execute_tools(&tool_calls).await?;
                
                // Add assistant message with tool calls
                self.session.append_message(Message {
                    role: Role::Assistant,
                    content: content,
                    tool_call_id: None,
                    tool_calls: Some(tool_calls.clone()),
                }).await?;
                
                // Add tool results
                for result in &results {
                    self.session.append_message(Message {
                        role: Role::Tool,
                        content: result.content.clone(),
                        tool_call_id: Some(result.tool_call_id.clone()),
                        tool_calls: None,
                    }).await?;
                }
                
                handle.submit_tool_results(results)?;
            }
            LoopStep::Done { content, .. } => {
                response_content.push_str(&content);
                break;
            }
            _ => {}
        }
    }
    
    // 5. Add assistant response to tree
    if !response_content.is_empty() {
        self.session.append_message(Message {
            role: Role::Assistant,
            content: response_content.clone(),
            tool_call_id: None,
            tool_calls: None,
        }).await?;
    }
    
    // 6. Auto checkpoint if needed
    self.auto_checkpoint_if_needed().await?;
    
    Ok(response_content)
}
```

---

## Changes to Existing Code

### Remove from LLMProvider

#### 1. Remove trait methods

```rust
// src/llm/provider.rs

#[async_trait::async_trait]
pub trait LLMProvider {
    // ✅ KEEP - These remain unchanged
    async fn chat(&self, prompt: &str) 
        -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;
    
    async fn chat_loop(
        &self,
        history: Vec<Message>,  // ← Still linear! Extracted from tree
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError>;
    
    fn state(&self) -> ProviderState;
    fn config(&self) -> ProviderConfig;
    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig));
    fn create(model: String, api_key: String) -> Result<Self, ProviderError> where Self: Sized;
    
    // ❌ REMOVE - Moved to Session layer
    // fn get_history(&self) -> Vec<Message>;
    // async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;
    // fn prompt_cache(&mut self, cache_prompt: String) -> Result<(), ProviderError>;
}
```

#### 2. Remove from implementations

```rust
// src/llm/openai.rs, anthropic.rs, gemini.rs

pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    
    // ❌ REMOVE
    // history: Arc<RwLock<Vec<Message>>>,
}

// ❌ REMOVE these implementations
// fn get_history(&self) -> Vec<Message> { ... }
// async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>> { ... }
// fn prune_tool_turns(messages: &mut Vec<ChatMessage>, max_turns: usize) { ... }
```

#### 3. Remove from ProviderConfig

```rust
// src/llm/provider.rs

pub struct ProviderConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub enable_reasoning: bool,
    pub system_prompt: Option<String>,
    pub stop_sequences: Vec<String>,
    pub extra_options: HashMap<String, serde_json::Value>,
    
    // ❌ REMOVE - Replaced by checkpoint system
    // pub max_tool_turns: Option<usize>,
}
```

#### 4. Update chat_loop implementations

**Current (OpenAI)**:
```rust
async fn chat_loop(&self, history: Vec<Message>, ...) -> Result<ChatLoopHandle> {
    // ... setup ...
    
    let provider_history = self.history.clone();  // ❌ Remove
    let mut current_history = history.clone();    // ❌ Remove
    
    // Inside loop:
    current_history.push(msg);  // ❌ Remove
    Self::prune_tool_turns(&mut messages, max_turns);  // ❌ Remove
    
    // At end:
    *provider_history.write() = current_history;  // ❌ Remove
}
```

**New (Stateless)**:
```rust
async fn chat_loop(&self, history: Vec<Message>, ...) -> Result<ChatLoopHandle> {
    // ✅ No internal history tracking
    // ✅ Just use input history directly
    
    let mut messages: Vec<ChatMessage> = history.iter()
        .map(Self::convert_message)
        .collect();
    
    // ✅ No pruning - tree layer handles context
    // ✅ No history accumulation
    
    // Just make API calls and stream results
}
```

---

## Implementation Tasks

### Phase 1: Core Tree Infrastructure ✅ COMPLETED

- [x] Create `src/history/` module structure
- [x] Define `Node`, `NodeId`, `SessionId`, `NodeKind` types
- [x] Define `NodeFlags`, `CheckpointData`, `CheckpointStats`
- [x] Implement `TreeStore` trait
- [x] Implement `MemoryStore` (HashMap-based in-memory storage)
- [x] Write unit tests for:
  - Node creation and validation
  - Parent-child relationships (get_children, path_to_root)
  - ULID ordering
  - Node-to-message conversion
  - Flag updates and pruning markers

**Files created:**
- `src/history/mod.rs` - Module exports
- `src/history/node.rs` - Core node types with ULID IDs (142 lines + tests)
- `src/history/storage.rs` - TreeStore trait interface (95 lines)
- `src/history/memory_store.rs` - In-memory implementation (470 lines with tests)
- `src/history/session.rs` - Session data structures (stub, 64 lines)

**Dependencies added:**
- `anyhow = "1.0"` - Error handling
- `ulid = { version = "1.2", features = ["serde"] }` - Time-ordered IDs

**Tests passing:** 9/9 (100%)

### Phase 2: Session Core API ✅ COMPLETED

- [x] Implement `Session` struct with runtime store integration
- [x] Implement `Session::new()` - create session with root node
- [x] Implement `Session::load()` - load existing session from storage
- [x] Implement `Session::append_message()` - add message to active leaf
- [x] Implement `Session::get_context()` - extract linear messages from active leaf
- [x] Implement `Session::get_context_from()` - extract from specific leaf
- [x] Implement `Session::create_checkpoint()` - create checkpoint metadata
- [x] Implement `Session::get_all_leaf_ids()` - get all leaf nodes
- [x] Implement `Session::get_branches()` - get branch metadata
- [x] Implement `Session::validate_tool_sandwich()` - validate tool call ordering
- [x] Write unit tests for:
  - Session creation
  - Message appending (sequential messages)
  - Context extraction (basic)
  - Context extraction with system prompt injection
  - Context extraction with checkpoint (stops at checkpoint)
  - Checkpoint creation and metadata
  - Leaf node enumeration

**Implementation details:**
- Full session lifecycle: create, load, append, persist
- Checkpoint-aware context extraction (stops at checkpoint nodes)
- Dynamic system prompt injection from config
- Tool Sandwich Constraint validation
- Automatic stats tracking (total_nodes, total_checkpoints)
- Head checkpoint cache with invalidation rules

**Tests passing:** 16/16 (100%) - 7 new session tests added

### Phase 3: Branching Support ✅ COMPLETED

- [x] Implement `Session::branch_from()` - create branch from node
- [x] Implement `Session::append_message_to()` - append to specific parent
- [x] Implement `Session::switch_to()` - switch active leaf
- [x] Implement `Session::get_branches()` - list all leaf nodes (completed in Phase 2)
- [x] Implement `Session::get_path_range()` - get nodes between two points
- [x] Write unit tests for:
  - Simple branch creation from middle of conversation
  - Multiple branches from same node (3 branches from node A)
  - Branch switching between leaves
  - Switch validation (non-leaf error handling)
  - Path range extraction
  - Path range validation (non-ancestor error handling)

**Implementation details:**
- `branch_from()` validates node and invalidates checkpoint cache
- `append_message_to()` allows appending to any node (creates branches)
- `switch_to()` validates leaf status before switching
- `get_path_range()` extracts node ranges between ancestors/descendants
- Full error handling for invalid operations

**Tests passing:** 22/22 (100%) - 6 new branching tests added

### Phase 4: Checkpoint System ✅ COMPLETED (in Phase 2)

- [x] Implement `Session::create_checkpoint()` - create checkpoint metadata (Phase 2)
- [x] Update `get_context()` to stop at checkpoints (Phase 2)
- [x] Implement checkpoint metadata and stats (Phase 2)
- [x] Write unit tests for checkpoints (Phase 2)
  - Checkpoint creation
  - Context extraction with checkpoints

**Note:** Checkpoint system was fully implemented in Phase 2 as checkpoint metadata (HashMap-based), not as separate tree nodes.

### Phase 5: Agent Layer ✅ COMPLETED

- [x] Create `src/agent/` module
- [x] Implement `Agent<P: LLMProvider>` struct (generic over provider)
- [x] Implement `Agent::new()` - create agent with session + provider
- [x] Implement `Agent::chat()` - main conversation loop with tool execution
- [x] Implement `Agent::branch_and_retry()` - branch from node and continue
- [x] Implement `Agent::checkpoint()` - manual checkpoint creation
- [x] Implement `Agent::auto_checkpoint_if_needed()` - auto checkpoint on message count
- [x] Implement `Agent::generate_summary()` - LLM-based summarization

**Implementation details:**
- Agent is generic over provider type (no Box<dyn> due to dyn-safety issues)
- Full tool execution loop with tool calls and results
- Automatic checkpoint creation based on message count threshold
- Integration with Session tree structure and ToolRegistry
- Stateless provider pattern - history managed by Session

**File created:**
- `src/agent/mod.rs` - Complete agent implementation (260 lines)

**Compiles successfully** - Ready for integration testing

### Phase 6: Provider Refactoring ✅ COMPLETED

**Date Completed:** 2026-01-06  
**Detailed Plan:** `doc/plan/PHASE6_PROVIDER_REFACTORING.md`  
**Completion Summary:** `doc/plan/PHASE6_COMPLETION_SUMMARY.md`

- [x] Remove `history: Arc<RwLock<Vec<Message>>>` from:
  - [x] `OpenAIProvider`
  - [x] `AnthropicProvider`
  - [x] `GeminiProvider`
- [x] Remove `get_history()` implementations
- [x] Remove `compact()` implementations
- [x] Remove `prune_tool_turns()` functions
- [x] Remove `max_tool_turns` from `ProviderConfig`
- [x] Update `chat_loop()` to be stateless:
  - [x] Remove history accumulation
  - [x] Remove pruning logic
  - [x] Just use input `history` parameter
- [x] Update all provider tests (76/76 passing)
- [x] Verify all examples still work (Agent-based, no changes needed)

**Key Metrics:**
- Lines removed: ~235 lines of history management code
- Tests: 76/76 passing (5 history tests removed)
- Breaking changes: Direct provider usage now requires Session
- Benefits: Clean separation, stateless providers, tree-based history enabled

### Phase 7: Storage Backends (Week 4)

- [ ] Implement `JSONLStore` (PRIMARY - file-based):
  - Directory structure: `sessions/{session_id}/`
  - `session.json` - session metadata
  - `nodes.jsonl` - append-only node log
  - `index.json` - optional fast lookup cache
  - Lazy loading (don't load all nodes into memory)
  - Async file I/O with tokio::fs
  - Atomic writes (write to temp file, then rename)
  - Background index rebuild on corruption
- [ ] Implement `MemoryStore` (for testing):
  - HashMap-based in-memory storage
  - Fast, no persistence
  - Good for unit tests
- [ ] Write storage backend tests
- [ ] Performance benchmarks for:
  - Node insertion (append to JSONL)
  - Context extraction (read + parse JSONL lines)
  - Branch listing (full tree scan)
  - Large tree handling (>10k nodes)
- [ ] Future: `SQLiteStore` (when database decision is made):
  - Schema design (nodes, sessions tables)
  - Indexes (session_id, parent_id, created_at)
  - Migration from JSONL to SQLite

### Phase 8: Advanced Features (Week 4-5)

- [ ] Implement pruning operations with protection rules:
  - Define **Protected Set** (see Pruning Protection Rules below)
  - `prune_orphaned_branches()` - remove unreachable nodes
  - `prune_ephemeral_nodes()` - remove old ephemeral nodes
  - Safe pruning validation (never prune nodes in Protected Set)
  - Pruning dry-run mode (preview what would be deleted)
- [ ] Implement vacuum/compaction for JSONL:
  - `Session::vacuum()` - rewrite nodes.jsonl, removing pruned nodes
  - Atomic file replacement (write to temp file, then rename)
  - Rebuild index after vacuum
  - Track file bloat ratio (deleted / total)
  - Auto-vacuum trigger (e.g., >30% bloat)
- [ ] Implement search and query:
  - `find_nodes()` with filters
  - Content search
  - Date range queries
- [ ] Implement visualization helpers:
  - Tree structure export
  - Branch graph generation
- [ ] Write tests for advanced features

#### Pruning Protection Rules

**Protected Set Definition**: The set of nodes that MUST NOT be pruned.

Protected Set = Union of:

1. **Active Paths**: For each active leaf (including all named branches):
   - Path from leaf to nearest checkpoint (inclusive)
   - Or path from leaf to root if no checkpoint exists

2. **All Checkpoint Nodes**: Every node where `kind == Checkpoint`
   - Checkpoints may be shared across branches
   - Never prune a checkpoint even if it appears "orphaned"

3. **Important Nodes**: Any node where `flags.important == true`

4. **Pinned Leaves** (optional): Explicitly marked leaf nodes to preserve
   - Allows keeping experimental branches without making them "active"

**Validation Algorithm**:

```rust
impl Session {
    /// Compute the protected set for safe pruning.
    pub async fn compute_protected_set(&self) -> Result<HashSet<NodeId>> {
        let mut protected = HashSet::new();
        
        // 1. Protect all active paths
        for leaf_id in self.get_all_leaf_ids().await? {
            let path = self.get_path_to_checkpoint_or_root(leaf_id).await?;
            for node in path {
                protected.insert(node.node_id.clone());
            }
        }
        
        // 2. Protect all checkpoints
        let checkpoints = self.store.find_nodes(
            self.session_id.clone(),
            NodeFilter {
                kinds: Some(vec![NodeKind::Checkpoint]),
                ..Default::default()
            }
        ).await?;
        for checkpoint in checkpoints {
            protected.insert(checkpoint.node_id.clone());
        }
        
        // 3. Protect important nodes
        let important = self.store.find_nodes(
            self.session_id.clone(),
            NodeFilter {
                flags: Some(NodeFlags { important: true, ..Default::default() }),
                ..Default::default()
            }
        ).await?;
        for node in important {
            protected.insert(node.node_id.clone());
        }
        
        // 4. Protect pinned leaves (if any)
        if let Some(pinned) = &self.pinned_leaves {
            for leaf_id in pinned {
                let path = self.get_path_to_checkpoint_or_root(leaf_id.clone()).await?;
                for node in path {
                    protected.insert(node.node_id.clone());
                }
            }
        }
        
        Ok(protected)
    }
    
    /// Safe pruning: delete only nodes NOT in protected set.
    pub async fn prune_safe(&mut self, candidate_nodes: Vec<NodeId>) -> Result<PruneResult> {
        let protected = self.compute_protected_set().await?;
        
        let mut pruned = Vec::new();
        let mut skipped = Vec::new();
        
        for node_id in candidate_nodes {
            if protected.contains(&node_id) {
                skipped.push(node_id);
            } else {
                self.store.delete_node(node_id.clone()).await?;
                pruned.push(node_id);
            }
        }
        
        Ok(PruneResult { pruned, skipped })
    }
}

#[derive(Debug)]
pub struct PruneResult {
    pub pruned: Vec<NodeId>,
    pub skipped: Vec<NodeId>,
}
```

**Example Scenarios**:

```
Scenario 1: Single active branch with checkpoint

root → A → B → C (checkpoint) → D → E → F (active_leaf)

Protected Set:
- F, E, D (active path to checkpoint)
- C (checkpoint itself)

Pruneable: A, B (if orphaned and not important)
```

```
Scenario 2: Multiple branches sharing a checkpoint

root → A → B → C (checkpoint) → D → E (leaf1, active)
                            └─→ X → Y (leaf2, inactive)

Protected Set:
- E, D (active path from leaf1)
- C (checkpoint)
- Y, X (if leaf2 is pinned OR important)

If leaf2 is not pinned/important: X, Y are pruneable
```

```
Scenario 3: Orphaned branch

root → A → B → C (active_leaf)
      └─→ X → Y → Z (orphaned leaf, no path to any active)

Protected Set:
- C, B, A (active path from C)

Pruneable: X, Y, Z (orphaned branch)
```

#### Vacuum/Compaction Strategy

**Problem**: File bloat after pruning

```
Timeline:
1. Insert 1000 nodes → nodes.jsonl has 1000 lines
2. Prune 500 nodes    → nodes.jsonl still has 1000 lines (marked deleted)
3. Insert 500 nodes   → nodes.jsonl has 1500 lines

Actual valid data: 1000 nodes
File contains: 1500 lines (33% bloat)
```

**Solution**: Vacuum (like `git gc`)

```rust
impl Session {
    /// Rewrite storage to remove deleted nodes (compaction).
    /// 
    /// This is similar to SQLite's VACUUM or git's gc.
    pub async fn vacuum(&mut self) -> Result<VacuumStats> {
        // 1. Compute protected set (all nodes we need to keep)
        let protected = self.compute_protected_set().await?;
        
        // 2. Write protected nodes to temp file
        let session_dir = self.get_session_dir();
        let nodes_file = session_dir.join("nodes.jsonl");
        let temp_file = session_dir.join("nodes.jsonl.tmp");
        
        let mut writer = tokio::fs::File::create(&temp_file).await?;
        let mut nodes_written = 0;
        
        for node_id in &protected {
            if let Some(node) = self.store.get_node(node_id.clone()).await? {
                let line = serde_json::to_string(&node)?;
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                nodes_written += 1;
            }
        }
        
        writer.sync_all().await?;
        drop(writer);
        
        // 3. Atomic replace
        tokio::fs::rename(&temp_file, &nodes_file).await?;
        
        // 4. Rebuild index
        self.rebuild_index().await?;
        
        Ok(VacuumStats {
            nodes_before: self.stats.total_nodes,
            nodes_after: nodes_written,
            bytes_freed: self.calculate_freed_bytes().await?,
        })
    }
    
    /// Calculate file bloat ratio.
    pub async fn bloat_ratio(&self) -> Result<f32> {
        let total_lines = self.count_jsonl_lines().await?;
        let protected = self.compute_protected_set().await?;
        
        if total_lines == 0 {
            return Ok(0.0);
        }
        
        let deleted_lines = total_lines - protected.len();
        Ok(deleted_lines as f32 / total_lines as f32)
    }
    
    /// Auto-vacuum if bloat exceeds threshold.
    pub async fn auto_vacuum_if_needed(&mut self) -> Result<Option<VacuumStats>> {
        let bloat = self.bloat_ratio().await?;
        
        if bloat > 0.3 {  // 30% threshold
            Ok(Some(self.vacuum().await?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct VacuumStats {
    pub nodes_before: u32,
    pub nodes_after: u32,
    pub bytes_freed: u64,
}
```

**When to vacuum**:
- Manual: `session.vacuum()` command
- Auto: After pruning if bloat > 30%
- Scheduled: Background task every N hours

#### Auto-Checkpoint on Large Content

**Use Case**: Reading large files with tools

```
Example scenario:
1. User: "Read main.go and analyze it"
2. Agent calls read_file tool
3. Tool returns 2000 lines of code (~8000 tokens)
4. This single message triggers auto-checkpoint
5. Checkpoint summary: "Read main.go (contains structs X, Y, Z...)"
6. Future messages use summary instead of full file content

Cost savings:
- Without checkpoint: Every API call includes 8000 tokens 💸💸💸
- With checkpoint: Every API call includes ~50 token summary 💰
```

**Implementation in Agent**:

```rust
impl Agent {
    async fn auto_checkpoint_if_needed(&mut self) -> Result<()> {
        let config = &self.session.config;
        
        // Guard: Don't checkpoint if active leaf already has a checkpoint
        if self.session.checkpoints.contains_key(&self.session.active_leaf_id) {
            return Ok(()); // Skip: this node already has a checkpoint
        }
        
        let context = self.session.get_context().await?;
        
        // Reason 1: Too many messages
        let too_many_messages = config.auto_checkpoint_every
            .map(|n| context.len() > n as usize)
            .unwrap_or(false);
        
        // Reason 2: Total tokens exceeded
        let total_tokens = context.iter()
            .map(|m| Self::estimate_tokens(&m.content))
            .sum::<usize>();
        let too_many_tokens = config.auto_checkpoint_token_limit
            .map(|limit| total_tokens > limit as usize)
            .unwrap_or(false);
        
        // Reason 3: Single large message (e.g., file read result)
        let has_large_message = config.auto_checkpoint_large_content
            .map(|limit| {
                context.iter().any(|m| Self::estimate_tokens(&m.content) > limit as usize)
            })
            .unwrap_or(false);
        
        if too_many_messages || too_many_tokens || has_large_message {
            log::info!("Auto-checkpointing: messages={}, tokens={}, large_message={}",
                too_many_messages, too_many_tokens, has_large_message);
            self.checkpoint().await?;
        }
        
        Ok(())
    }
    
    /// Rough token estimation (1 token ≈ 4 characters)
    fn estimate_tokens(text: &str) -> usize {
        text.len() / 4
    }
}
```

**Checkpoint Strategy Selection**:

```rust
// In Agent::checkpoint()
async fn checkpoint(&mut self) -> Result<NodeId> {
    let context = self.session.get_context().await?;
    
    // Detect why we're checkpointing
    let has_large_content = context.iter()
        .any(|m| Self::estimate_tokens(&m.content) > 5000);
    
    let summary = if has_large_content {
        // Focused summary for large content
        self.generate_focused_summary(&context).await?
    } else {
        // General conversation summary
        self.generate_summary(&context).await?
    };
    
    // ... create checkpoint
}

async fn generate_focused_summary(&self, context: &[Message]) -> Result<String> {
    // Extract key information from large messages
    let prompt = format!(
        "Summarize the following, focusing on key data structures and important details:\n{:?}",
        context
    );
    // ... call LLM
}
```

### Phase 9: CLI Integration (Week 5)

- [ ] Update `main.rs` to use Agent layer
- [ ] Add CLI commands:
  - `branch list` - show all branches
  - `branch switch <id>` - switch to branch
  - `branch from <node_id>` - create branch
  - `checkpoint create` - manual checkpoint
  - `checkpoint list` - show checkpoints
  - `history show [--from <node_id>]` - show context
- [ ] Update examples to use Agent:
  - `examples/openai_basic.rs`
  - `examples/simple_agent.rs`
  - `examples/interactive_agent.rs`
- [ ] Create new examples:
  - `examples/branching_demo.rs`
  - `examples/checkpoint_demo.rs`

### Phase 10: Documentation (Week 5-6)

- [ ] Write API documentation (rustdoc):
  - `history` module
  - `agent` module
  - All public APIs
- [ ] Write user guide:
  - Concepts (session, node, checkpoint)
  - Basic usage with Agent
  - Branching workflows
  - Checkpoint strategies
- [ ] Write migration guide:
  - Changes from old API
  - How to update existing code
  - Breaking changes summary
- [ ] Update main README:
  - Tree model overview
  - Quick start with Agent
  - Architecture diagram

---

## Dependencies

### New Dependencies

```toml
[dependencies]
# Core (already have these)
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"

# ULID generation for NodeId/SessionId (time-ordered, sortable)
ulid = { version = "1.0", features = ["serde"] }

# For time handling
chrono = { version = "0.4", features = ["serde"] }

# For async file I/O (JSONL store)
tokio = { version = "1", features = ["fs", "io-util"] }

# For SQLite storage (optional, future)
rusqlite = { version = "0.31", features = ["bundled"], optional = true }
```

### Feature Flags

```toml
[features]
default = ["jsonl-store"]
memory-store = []
jsonl-store = []  # File-based storage (default for now)
sqlite-store = ["dep:rusqlite"]  # Future: proper database
all-stores = ["memory-store", "jsonl-store", "sqlite-store"]
```

---

## Testing Strategy

### Unit Tests

**Core Tree Operations**
- Node creation with all kinds (Root, Message, Tool, Checkpoint)
- Parent-child linking
- Sibling ordering (seq field)
- Node flags (important, ephemeral, hidden)

**Session Operations**
- Session creation
- Message appending
- Context extraction (simple, with checkpoints, with hidden nodes)
- Branch creation and switching
- Checkpoint creation and coverage

### Integration Tests

**Full Conversation Flow**
- User message → Agent → LLM → Assistant response → Tree storage
- Multi-turn conversation
- Tool calling with tree storage
- Context extraction and provider call

**Branching Scenarios**
- Single branch
- Multiple branches from same node
- Nested branching
- Switch between branches

**Checkpoint Scenarios**
- Create checkpoint manually
- Auto checkpoint on message count
- Context extraction stops at checkpoint
- Multiple checkpoints in path

### Performance Tests

**Large Tree Handling**
- 1k nodes: context extraction time
- 10k nodes: branch listing time
- 100k nodes: storage backend performance

**Memory Usage**
- Session with 1k messages
- 10 concurrent sessions
- Large checkpoint summaries

### Storage Backend Tests

**MemoryStore**
- All CRUD operations
- Query performance
- Concurrent access

**SQLiteStore**
- Schema creation
- Indexes effectiveness
- Transaction handling
- Migration scenarios

---

## Success Criteria

- [ ] All `LLMProvider` implementations remain linear (no tree awareness)
- [ ] Can branch from any point in conversation history
- [ ] Can replay conversation from any node
- [ ] Checkpoints reduce context size by >50% for long conversations
- [ ] Context extraction is O(depth) from leaf to checkpoint
- [ ] All existing examples work with Agent layer
- [ ] Storage backends are pluggable
- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] Documentation complete
- [ ] Migration guide available

---

## Risks & Mitigations

### Risk 1: Performance Degradation on Large Trees
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: 
- Implement efficient indexes in storage backends
- Cache path-to-root for active leaves
- Benchmark with realistic conversation sizes
- Add pagination for branch listing

### Risk 2: Complex Migration from Current Code
**Likelihood**: Medium  
**Impact**: High  
**Mitigation**:
- Comprehensive migration guide
- Keep changes localized to history management
- Maintain provider API stability
- Phase rollout (memory store first, then SQLite)

### Risk 3: Checkpoint Summary Quality
**Likelihood**: Low  
**Impact**: Medium  
**Mitigation**:
- Use same LLM provider for summarization
- Include key information extraction prompts
- Allow manual checkpoint editing
- Store full nodes even after checkpoint (for recovery)

### Risk 4: Storage Corruption
**Likelihood**: Low  
**Impact**: High  
**Mitigation**:
- Use transactions in SQLite
- Validate tree invariants on load
- Implement backup/restore
- JSONL store as human-readable backup

### Risk 5: Concurrency Issues
**Likelihood**: Low  
**Impact**: High  
**Mitigation**:
- Single writer per session (mutex/lock)
- Optimistic concurrency for session updates
- Version field on Session for CAS
- Clear concurrency model documentation

---

## Open Questions

### 1. Checkpoint Strategy Selection
**Question**: Should auto-checkpoint use message count, token count, or both?  
**Options**:
- A. Message count only (simple, predictable)
- B. Token count only (more accurate, requires estimation)
- C. Both (whichever hits first)

**Decision**: Start with message count (A), add token estimation later

### 2. Node Deletion vs Hidden Flag
**Question**: Should pruning delete nodes or just mark them hidden?  
**Options**:
- A. Hard delete (saves storage, risky)
- B. Soft delete with hidden flag (safer, uses more storage)
- C. Configurable per session

**Decision**: Soft delete with hidden flag (B), consider hard delete for ephemeral

### 3. Multi-Session Management
**Question**: How should Agent handle multiple sessions?  
**Options**:
- A. One session per Agent (simple)
- B. Agent can switch sessions
- C. SessionManager as separate component

**Decision**: One session per Agent (A), create new Agent for new session

### 4. Checkpoint Reuse Across Branches
**Question**: Can multiple branches share the same checkpoint?  
**Options**:
- A. Yes, checkpoint is shared (saves storage, more complex)
- B. No, each branch has its own checkpoints (simpler, uses more storage)

**Decision**: Yes (A), checkpoints are shared nodes in the tree

### 5. Tool Result Storage
**Question**: Should tool results be separate nodes or embedded in parent?  
**Options**:
- A. Separate Tool nodes (current plan, more flexible)
- B. Embedded in Assistant message (simpler, less flexible)

**Decision**: Separate Tool nodes (A), better for branching/replay

---

## Future Enhancements

### Phase 2 Features (Post-MVP)

- [ ] Checkpoint editing and refinement
- [ ] Merge branches (combine two conversation paths)
- [ ] Export conversation as markdown/JSON
- [ ] Import conversation from other formats
- [ ] Search across all sessions
- [ ] Tag-based session organization
- [ ] Session templates
- [ ] Automated pruning policies
- [ ] Tree visualization UI
- [ ] Undo/redo operations

### Advanced Storage Features

- [ ] Incremental backup/restore
- [ ] Cloud storage backends (S3, etc.)
- [ ] Compression for old nodes
- [ ] Distributed storage for multi-agent systems
- [ ] Real-time sync between instances

### Agent Enhancements

- [ ] Multi-agent collaboration (shared sessions)
- [ ] Automatic branch exploration (what-if scenarios)
- [ ] Conversation quality metrics
- [ ] A/B testing different approaches
- [ ] Replay with different providers

---

## References

### Inspiration
- Git's commit and branch model
- Jupyter notebook checkpoints
- Undo/redo systems in editors

### Technical
- [Tree structures in databases](https://www.postgresql.org/docs/current/ltree.html)
- [Materialized path pattern](https://docs.mongodb.com/manual/tutorial/model-tree-structures-with-materialized-paths/)
- [SQLite recursive queries](https://www.sqlite.org/lang_with.html)

### Related Work
- LangChain memory systems
- Semantic Kernel conversation history
- AutoGen conversation management

---

## Timeline Estimate

**Total Effort**: 5-6 weeks (1 developer full-time)

- **Week 1**: Core tree + Session basics (Phases 1-2)
- **Week 2**: Branching + Checkpoints (Phases 3-4)
- **Week 3**: Agent + Provider refactoring (Phases 5-6)
- **Week 4**: Storage backends + Advanced features (Phases 7-8)
- **Week 5**: CLI + Examples (Phase 9)
- **Week 6**: Documentation + Polish (Phase 10)

**Milestones**:
- ✅ End of Week 1: Basic tree working with in-memory storage
- ✅ End of Week 2: Branching and checkpoints functional
- ✅ End of Week 3: Agent layer complete, providers refactored
- ✅ End of Week 4: SQLite storage working, advanced features done
- ✅ End of Week 5: CLI updated, examples working
- ✅ End of Week 6: All documentation complete, ready for use
