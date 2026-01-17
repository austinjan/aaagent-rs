use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use super::compressor::{CompressionConfig, ContextCompressor};
use super::node::{
    new_node_id, new_session_id, now, ArchivedToolResult, CheckpointData, ContentType, Node,
    NodeFlags, NodeId, NodeKind,
};
use super::storage::TreeStore;
use super::validator::MessageValidator;
use crate::llm::{Message, Role};

pub type SessionId = String;

/// Session represents a conversation workspace
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: SessionId,
    pub created_at: i64,
    pub updated_at: i64,
    pub root_node_id: NodeId,
    pub active_leaf_id: NodeId,

    /// Cache of the nearest checkpoint on path from active_leaf to root
    pub head_checkpoint_id: Option<NodeId>,

    // Metadata
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub config: SessionConfig,
    pub stats: SessionStats,

    /// Checkpoints: node_id -> checkpoint data
    pub checkpoints: HashMap<NodeId, CheckpointData>,

    /// Optional: pinned leaf nodes to protect from pruning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_leaves: Option<Vec<NodeId>>,

    /// Archived tool results for on-demand retrieval
    /// Used when tool results are compressed from context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_tool_results: Option<HashMap<String, ArchivedToolResult>>,

    /// Future extension point: arbitrary session metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    // Runtime (not serialized)
    #[serde(skip)]
    #[serde(default)]
    store: Option<Arc<dyn TreeStore>>,
}

// Manual Debug implementation to handle trait object
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("session_id", &self.session_id)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("root_node_id", &self.root_node_id)
            .field("active_leaf_id", &self.active_leaf_id)
            .field("head_checkpoint_id", &self.head_checkpoint_id)
            .field("name", &self.name)
            .field("tags", &self.tags)
            .field("config", &self.config)
            .field("stats", &self.stats)
            .field("checkpoints", &self.checkpoints)
            .field("pinned_leaves", &self.pinned_leaves)
            .field("archived_tool_results", &self.archived_tool_results)
            .field("metadata", &self.metadata)
            .field("store", &"<TreeStore>")
            .finish()
    }
}

/// Checkpoint configuration for automatic context summarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Auto-checkpoint after N user turns (conversation rounds)
    /// Counts only User messages, not tool calls/results
    /// Example: 10 means checkpoint after 10 user questions
    pub every_n_turns: Option<u32>,

    /// Auto-checkpoint when context exceeds N tokens
    pub token_limit: Option<u32>,

    /// Auto-checkpoint when a single message exceeds N tokens
    pub large_content_threshold: Option<u32>,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            every_n_turns: Some(50),
            token_limit: Some(100_000),
            large_content_threshold: Some(5_000),
        }
    }
}

/// Context optimization configuration
/// Combines tool result compression and checkpoint strategies
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextOptimizationConfig {
    /// Tool result compression settings
    pub compression: CompressionConfig,

    /// Checkpoint settings for aggressive context reduction
    pub checkpoint: CheckpointConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// System prompt injected at the start of context
    pub system_prompt: Option<String>,

    /// Maximum context tokens before forcing checkpoint
    pub max_context_tokens: u32,

    /// Prune ephemeral branches after N days
    pub prune_ephemeral_after_days: u32,

    /// Context optimization settings (compression + checkpoint)
    pub optimization: ContextOptimizationConfig,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            system_prompt: None,
            max_context_tokens: 200_000,
            prune_ephemeral_after_days: 7,
            optimization: ContextOptimizationConfig::default(),
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

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub leaf_id: NodeId,
    pub depth: u32,
    pub last_updated: i64,
    pub is_active: bool,
}

impl Session {
    /// Create a new session with a root node
    pub async fn new(store: Arc<dyn TreeStore>, config: SessionConfig) -> Result<Self> {
        let session_id = new_session_id();
        let root_node_id = new_node_id();
        let created_at = now();

        // Create root node
        let root_node = Node {
            node_id: root_node_id.clone(),
            session_id: session_id.clone(),
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: String::new(),
            created_at,
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };

        // Insert root node
        store.insert_node(root_node).await?;

        // Create session
        let mut session = Self {
            session_id: session_id.clone(),
            created_at,
            updated_at: created_at,
            root_node_id: root_node_id.clone(),
            active_leaf_id: root_node_id.clone(),
            head_checkpoint_id: None,
            name: None,
            tags: Vec::new(),
            config,
            stats: SessionStats::default(),
            checkpoints: HashMap::new(),
            pinned_leaves: None,
            archived_tool_results: Some(HashMap::new()),
            metadata: None,
            store: Some(store.clone()),
        };

        session.stats.total_nodes = 1;
        session.stats.active_branches = 1;

        // Persist session
        store.create_session(session.clone()).await?;

        Ok(session)
    }

    /// Load an existing session from storage
    pub async fn load(store: Arc<dyn TreeStore>, session_id: SessionId) -> Result<Self> {
        let mut session = store
            .get_session(session_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

        session.store = Some(store);
        Ok(session)
    }

    /// Attach a store to an existing session
    pub fn set_store(&mut self, store: Arc<dyn TreeStore>) {
        self.store = Some(store);
    }

    /// Get the store (helper method)
    fn store(&self) -> Result<&Arc<dyn TreeStore>> {
        self.store
            .as_ref()
            .ok_or_else(|| anyhow!("Session not connected to store"))
    }

    /// Append a message node to the active leaf
    pub async fn append_message(&mut self, msg: Message) -> Result<NodeId> {
        self.append_message_to(self.active_leaf_id.clone(), msg)
            .await
    }

    /// Append a message node to a specific parent node and switch active leaf
    pub async fn append_message_to(&mut self, parent_id: NodeId, msg: Message) -> Result<NodeId> {
        let store = self.store()?.clone();

        // Use session-scoped API to count children efficiently
        let children_count = store
            .count_children_in_session(self.session_id.clone(), parent_id.clone())
            .await?;
        let seq = children_count as u32 + 1;

        // Create new node
        let node = Node {
            node_id: new_node_id(),
            session_id: self.session_id.clone(),
            parent_id: Some(parent_id),
            kind: NodeKind::Message,
            role: Some(msg.role.clone()),
            content_type: ContentType::Text,
            content: msg.content.clone(),
            created_at: now(),
            seq,
            flags: NodeFlags::default(),
            tool_call_id: msg.tool_call_id.clone(),
            tool_calls: msg.tool_calls.clone(),
            pruned_at: None,
            metadata: None,
        };

        let node_id = node.node_id.clone();
        store.insert_node(node).await?;

        // Update active leaf
        self.active_leaf_id = node_id.clone();
        self.updated_at = now();
        self.stats.total_nodes += 1;

        // Invalidate checkpoint cache
        self.head_checkpoint_id = None;

        // Persist session
        store.update_session(self).await?;

        Ok(node_id)
    }

    /// Extract linear context from active leaf to root/checkpoint
    pub async fn get_context(&mut self) -> Result<Vec<Message>> {
        let leaf_id = self.active_leaf_id.clone();
        self.get_context_from(leaf_id).await
    }

    /// Extract linear context from specific leaf
    pub async fn get_context_from(&mut self, leaf_id: NodeId) -> Result<Vec<Message>> {
        let store = self.store()?;

        // Build path to root using session-scoped API
        let path_nodes = store.get_path_to_root_internal(leaf_id.clone()).await?;

        let mut messages = Vec::new();

        // Step 1: Extract messages from tree
        for node in &path_nodes {
            // Check if this node has a checkpoint
            if let Some(checkpoint) = self.checkpoints.get(&node.node_id) {
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
        }

        messages.reverse();

        // Step 2: Inject dynamic system prompt from config
        if let Some(system_prompt) = &self.config.system_prompt {
            messages.insert(
                0,
                Message {
                    role: Role::System,
                    content: system_prompt.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            );
        }

        // Step 3: Compress tool results based on configuration
        let compressor = ContextCompressor::new(self.config.optimization.compression.clone());
        let archived = self.archived_tool_results.get_or_insert_with(HashMap::new);
        messages = compressor.compress(messages, archived);

        // Step 4: Validate Tool Sandwich Constraint
        MessageValidator::validate_tool_sandwich(&messages)?;

        Ok(messages)
    }

    /// Create a checkpoint at a specific node (metadata, not a new node)
    pub async fn create_checkpoint(
        &mut self,
        node_id: NodeId,
        summary: String,
        strategy: &str,
    ) -> Result<()> {
        let store = self.store()?.clone();

        // Verify node exists and belongs to this session using session-scoped API
        let _node = store
            .get_node_in_session(self.session_id.clone(), node_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found in session {}", node_id, self.session_id))?;

        // Create checkpoint data
        let checkpoint = CheckpointData {
            summary,
            created_at: now(),
            strategy: Some(strategy.to_string()),
            stats: None,
            extensions: None,
        };

        // Store checkpoint
        self.checkpoints.insert(node_id.clone(), checkpoint);
        self.stats.total_checkpoints += 1;

        // Update head checkpoint cache if this is on the active path
        if node_id == self.active_leaf_id {
            self.head_checkpoint_id = Some(node_id.clone());
        }

        // Persist session
        store.update_session(self).await?;

        Ok(())
    }

    /// Check if auto-checkpoint should be triggered based on current context
    ///
    /// Returns Some((strategy_name)) if checkpoint should be created, None otherwise.
    /// This does NOT create the checkpoint - caller must call create_checkpoint with a summary.
    pub async fn should_auto_checkpoint(&self) -> Result<Option<&'static str>> {
        let context = self.get_context_internal().await?;

        // Check 1: Too many user turns
        if let Some(turn_limit) = self.config.optimization.checkpoint.every_n_turns {
            let user_turn_count = context.iter().filter(|m| m.role == Role::User).count();
            if user_turn_count > turn_limit as usize {
                return Ok(Some("auto_turns"));
            }
        }

        // Check 2: Context too large (by character count as proxy for tokens)
        // Rough estimate: 4 chars ≈ 1 token
        if let Some(token_limit) = self.config.optimization.checkpoint.token_limit {
            let total_chars: usize = context.iter().map(|m| m.content.len()).sum();
            let estimated_tokens = total_chars / 4;
            if estimated_tokens > token_limit as usize {
                return Ok(Some("auto_token_limit"));
            }
        }

        // Check 3: Single large message
        if let Some(large_threshold) = self.config.optimization.checkpoint.large_content_threshold {
            for msg in &context {
                let estimated_tokens = msg.content.len() / 4;
                if estimated_tokens > large_threshold as usize {
                    return Ok(Some("auto_large_content"));
                }
            }
        }

        Ok(None)
    }

    /// Internal get_context without compression (for checkpoint checking)
    async fn get_context_internal(&self) -> Result<Vec<Message>> {
        let store = self.store()?;

        // Build path to root using session-scoped API
        let path_nodes = store
            .get_path_to_root_internal(self.active_leaf_id.clone())
            .await?;

        let mut messages = Vec::new();

        for node in &path_nodes {
            if self.checkpoints.contains_key(&node.node_id) {
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
        }

        messages.reverse();
        Ok(messages)
    }

    /// Check if this node already has a checkpoint
    pub fn has_checkpoint(&self, node_id: &NodeId) -> bool {
        self.checkpoints.contains_key(node_id)
    }

    /// Archive a tool result for later retrieval
    ///
    /// This is used when compressing tool results from context. The full content
    /// is stored in the archive so it can be retrieved via recall_tool_result tool.
    pub fn archive_tool_result(&mut self, result: ArchivedToolResult) {
        let archive = self.archived_tool_results.get_or_insert_with(HashMap::new);
        archive.insert(result.tool_call_id.clone(), result);
    }

    /// Retrieve an archived tool result by tool_call_id
    pub fn get_archived_tool_result(&self, tool_call_id: &str) -> Option<&ArchivedToolResult> {
        self.archived_tool_results
            .as_ref()
            .and_then(|archive| archive.get(tool_call_id))
    }

    /// Get all archived tool result IDs
    pub fn get_archived_tool_result_ids(&self) -> Vec<String> {
        self.archived_tool_results
            .as_ref()
            .map(|archive| archive.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Save a key-value pair to session metadata
    pub fn set_metadata<T: Serialize>(&mut self, key: &str, value: &T) -> Result<()> {
        let metadata = self.metadata.get_or_insert_with(|| serde_json::json!({}));

        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(key.to_string(), serde_json::to_value(value)?);
        } else {
            // Replace with object if not already an object
            *metadata = serde_json::json!({ key: serde_json::to_value(value)? });
        }

        self.updated_at = now();
        Ok(())
    }

    /// Get a value from session metadata
    pub fn get_metadata<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.metadata
            .as_ref()?
            .as_object()?
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Remove a key from session metadata
    pub fn remove_metadata(&mut self, key: &str) -> Option<serde_json::Value> {
        let metadata = self.metadata.as_mut()?;
        let obj = metadata.as_object_mut()?;
        let removed = obj.remove(key);

        if removed.is_some() {
            self.updated_at = now();
        }

        removed
    }

    /// Check if a metadata key exists
    pub fn has_metadata(&self, key: &str) -> bool {
        self.metadata
            .as_ref()
            .and_then(|m| m.as_object())
            .map(|obj| obj.contains_key(key))
            .unwrap_or(false)
    }

    /// Get all leaf node IDs in this session
    pub async fn get_all_leaf_ids(&self) -> Result<Vec<NodeId>> {
        let store = self.store()?;

        // Get all nodes in session
        let all_nodes = store
            .find_nodes(self.session_id.clone(), Default::default())
            .await?;

        // Find nodes with no children using session-scoped API
        let mut leaf_ids = Vec::new();
        for node in &all_nodes {
            if node.kind == NodeKind::Root {
                continue; // Root is never a leaf
            }

            let children_count = store
                .count_children_in_session(self.session_id.clone(), node.node_id.clone())
                .await?;
            if children_count == 0 {
                leaf_ids.push(node.node_id.clone());
            }
        }

        Ok(leaf_ids)
    }

    /// Get all branches (leaf nodes with metadata)
    pub async fn get_branches(&self) -> Result<Vec<BranchInfo>> {
        let store = self.store()?;
        let leaf_ids = self.get_all_leaf_ids().await?;

        let mut branches = Vec::new();
        for leaf_id in leaf_ids {
            // Calculate depth by walking to root
            let path = store.get_path_to_root_internal(leaf_id.clone()).await?;
            let depth = path.len() as u32;

            // Get last updated time (leaf's created_at)
            let leaf_node = store
                .get_node(leaf_id.clone())
                .await?
                .ok_or_else(|| anyhow!("Leaf node {} not found", leaf_id))?;

            branches.push(BranchInfo {
                leaf_id: leaf_id.clone(),
                depth,
                last_updated: leaf_node.created_at,
                is_active: leaf_id == self.active_leaf_id,
            });
        }

        Ok(branches)
    }

    /// Create a branch from a specific node
    ///
    /// This creates a new conversation path starting from the given node.
    /// The active leaf is NOT automatically switched to the new branch.
    ///
    /// # Example
    /// ```text
    /// // Original: root -> A -> B -> C (active)
    /// let new_leaf = session.branch_from(node_a_id).await?;
    /// // Now: root -> A -> B -> C (still active)
    ///              └-> (new_leaf, not active)
    /// ```
    pub async fn branch_from(&mut self, node_id: NodeId) -> Result<NodeId> {
        let store = self.store()?.clone();

        // Verify node exists and belongs to this session using session-scoped API
        let _node = store
            .get_node_in_session(self.session_id.clone(), node_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found in session {}", node_id, self.session_id))?;

        // The new branch point is just the node_id itself
        // No new node is created - caller should append_message to create branch content

        // Invalidate checkpoint cache (active path might change later)
        self.head_checkpoint_id = None;

        // Update stats - we'll increment active_branches when a message is added
        // For now, just return the node_id as the branch point

        Ok(node_id)
    }

    /// Switch active leaf to a different branch
    ///
    /// Changes which branch is currently "active" (HEAD).
    ///
    /// # Example
    /// ```text
    /// // Before: root -> A -> B (active)
    ///               └-> X -> Y
    /// session.switch_to(node_y_id).await?;
    /// // After: root -> A -> B
    ///              └-> X -> Y (active)
    /// ```
    pub async fn switch_to(&mut self, leaf_id: NodeId) -> Result<()> {
        let store = self.store()?.clone();

        // Verify leaf exists and belongs to this session using session-scoped API
        let _leaf_node = store
            .get_node_in_session(self.session_id.clone(), leaf_id.clone())
            .await?
            .ok_or_else(|| anyhow!("Node {} not found in session {}", leaf_id, self.session_id))?;

        // Verify it's actually a leaf (no children) using session-scoped API
        let children_count = store
            .count_children_in_session(self.session_id.clone(), leaf_id.clone())
            .await?;
        if children_count > 0 {
            return Err(anyhow!(
                "Node {} is not a leaf (has {} children)",
                leaf_id,
                children_count
            ));
        }

        // Switch active leaf
        self.active_leaf_id = leaf_id;
        self.updated_at = now();

        // Invalidate checkpoint cache
        self.head_checkpoint_id = None;

        // Persist session
        store.update_session(self).await?;

        Ok(())
    }

    /// Get path range between two nodes
    ///
    /// Returns all nodes from `from_node` to `to_node` (inclusive).
    /// The nodes must be on the same path (ancestor-descendant relationship).
    ///
    /// # Arguments
    /// - `from_node`: Starting node (must be ancestor of to_node)
    /// - `to_node`: Ending node (must be descendant of from_node)
    ///
    /// # Returns
    /// Vec of nodes from from_node to to_node in chronological order
    pub async fn get_path_range(&self, from_node: NodeId, to_node: NodeId) -> Result<Vec<Node>> {
        let store = self.store()?;

        // Get path from to_node to root
        let full_path = store.get_path_to_root_internal(to_node.clone()).await?;

        // Find from_node in the path
        let from_index = full_path
            .iter()
            .position(|n| n.node_id == from_node)
            .ok_or_else(|| anyhow!("Node {} is not an ancestor of {}", from_node, to_node))?;

        // Extract range (from to_node to from_node)
        let mut range: Vec<Node> = full_path[..=from_index].to_vec();

        // Reverse to get chronological order (from_node -> to_node)
        range.reverse();

        Ok(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::MemoryStore;

    #[tokio::test]
    async fn test_session_creation() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let session = Session::new(store.clone(), config).await.unwrap();

        assert_eq!(session.stats.total_nodes, 1); // Root node
        assert_eq!(session.stats.active_branches, 1);
        assert_eq!(session.root_node_id, session.active_leaf_id);
    }

    #[tokio::test]
    async fn test_append_message() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Append user message
        let msg1 = Message {
            role: Role::User,
            content: "Hello".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        let node1_id = session.append_message(msg1).await.unwrap();

        assert_eq!(session.stats.total_nodes, 2);
        assert_eq!(session.active_leaf_id, node1_id);

        // Append assistant message
        let msg2 = Message {
            role: Role::Assistant,
            content: "Hi there!".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        let node2_id = session.append_message(msg2).await.unwrap();

        assert_eq!(session.stats.total_nodes, 3);
        assert_eq!(session.active_leaf_id, node2_id);
    }

    #[tokio::test]
    async fn test_get_context() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Append messages
        session
            .append_message(Message {
                role: Role::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Hi there!".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Get context
        let context = session.get_context().await.unwrap();

        assert_eq!(context.len(), 2);
        assert_eq!(context[0].role, Role::User);
        assert_eq!(context[0].content, "Hello");
        assert_eq!(context[1].role, Role::Assistant);
        assert_eq!(context[1].content, "Hi there!");
    }

    #[tokio::test]
    async fn test_get_context_with_system_prompt() {
        let store = Arc::new(MemoryStore::new());
        let mut config = SessionConfig::default();
        config.system_prompt = Some("You are a helpful assistant.".to_string());

        let mut session = Session::new(store.clone(), config).await.unwrap();

        session
            .append_message(Message {
                role: Role::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let context = session.get_context().await.unwrap();

        assert_eq!(context.len(), 2);
        assert_eq!(context[0].role, Role::System);
        assert_eq!(context[0].content, "You are a helpful assistant.");
        assert_eq!(context[1].role, Role::User);
        assert_eq!(context[1].content, "Hello");
    }

    #[tokio::test]
    async fn test_checkpoint() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Append messages
        let node1_id = session
            .append_message(Message {
                role: Role::User,
                content: "Message 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Response 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Create checkpoint at node1
        session
            .create_checkpoint(
                node1_id.clone(),
                "Summary of conversation so far".to_string(),
                "manual",
            )
            .await
            .unwrap();

        assert_eq!(session.stats.total_checkpoints, 1);
        assert!(session.checkpoints.contains_key(&node1_id));
    }

    #[tokio::test]
    async fn test_get_context_with_checkpoint() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Append messages
        let node1_id = session
            .append_message(Message {
                role: Role::User,
                content: "Message 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Response 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::User,
                content: "Message 2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Create checkpoint at node1
        session
            .create_checkpoint(
                node1_id,
                "Previous conversation summary".to_string(),
                "manual",
            )
            .await
            .unwrap();

        // Get context - should stop at checkpoint
        let context = session.get_context().await.unwrap();

        // Should have: checkpoint summary + Response 1 + Message 2
        // Walking backwards: Message 2 -> Response 1 -> Message 1 (checkpoint, stop)
        assert_eq!(context.len(), 3);
        assert_eq!(context[0].role, Role::System);
        assert_eq!(context[0].content, "Previous conversation summary");
        assert_eq!(context[1].role, Role::Assistant);
        assert_eq!(context[1].content, "Response 1");
        assert_eq!(context[2].role, Role::User);
        assert_eq!(context[2].content, "Message 2");
    }

    #[tokio::test]
    async fn test_get_all_leaf_ids() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Append one message
        let leaf_id = session
            .append_message(Message {
                role: Role::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let leaves = session.get_all_leaf_ids().await.unwrap();

        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0], leaf_id);
    }

    #[tokio::test]
    async fn test_branch_from() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Create initial path: root -> A -> B -> C
        let node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let node_c_id = session
            .append_message(Message {
                role: Role::User,
                content: "C".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Branch from A
        let branch_point = session.branch_from(node_a_id.clone()).await.unwrap();
        assert_eq!(branch_point, node_a_id);

        // Active leaf should still be C
        assert_eq!(session.active_leaf_id, node_c_id);

        // Now append to create actual branch content from A
        let node_x_id = session
            .append_message_to(
                node_a_id.clone(),
                Message {
                    role: Role::Assistant,
                    content: "X".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await
            .unwrap();

        // Now we have two branches
        let leaves = session.get_all_leaf_ids().await.unwrap();
        assert_eq!(leaves.len(), 2);
        assert!(leaves.contains(&node_c_id));
        assert!(leaves.contains(&node_x_id));
    }

    #[tokio::test]
    async fn test_switch_to() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Create two branches
        let node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let node_b_id = session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Branch from A
        let node_x_id = session
            .append_message_to(
                node_a_id.clone(),
                Message {
                    role: Role::Assistant,
                    content: "X".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await
            .unwrap();

        // Currently at X
        assert_eq!(session.active_leaf_id, node_x_id);

        // Switch to B
        session.switch_to(node_b_id.clone()).await.unwrap();
        assert_eq!(session.active_leaf_id, node_b_id);

        // Get context should now be from B branch
        let context = session.get_context().await.unwrap();
        assert_eq!(context.len(), 2);
        assert_eq!(context[0].content, "A");
        assert_eq!(context[1].content, "B");
    }

    #[tokio::test]
    async fn test_switch_to_non_leaf_fails() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        let node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Try to switch to A (which has child B)
        let result = session.switch_to(node_a_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a leaf"));
    }

    #[tokio::test]
    async fn test_get_path_range() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Create path: root -> A -> B -> C -> D
        let _node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let node_b_id = session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let _node_c_id = session
            .append_message(Message {
                role: Role::User,
                content: "C".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let node_d_id = session
            .append_message(Message {
                role: Role::Assistant,
                content: "D".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Get range from B to D
        let range = session.get_path_range(node_b_id, node_d_id).await.unwrap();

        assert_eq!(range.len(), 3); // B, C, D
        assert_eq!(range[0].content, "B");
        assert_eq!(range[1].content, "C");
        assert_eq!(range[2].content, "D");
    }

    #[tokio::test]
    async fn test_get_path_range_invalid() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Create two separate branches
        let node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        let node_b_id = session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Branch from A
        let node_x_id = session
            .append_message_to(
                node_a_id.clone(),
                Message {
                    role: Role::Assistant,
                    content: "X".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await
            .unwrap();

        // Try to get range from B to X (not on same path)
        let result = session.get_path_range(node_b_id, node_x_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an ancestor"));
    }

    #[tokio::test]
    async fn test_multiple_branches_from_same_node() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();

        let mut session = Session::new(store.clone(), config).await.unwrap();

        // Create initial node A
        let node_a_id = session
            .append_message(Message {
                role: Role::User,
                content: "A".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Branch 1: A -> B
        let node_b_id = session
            .append_message(Message {
                role: Role::Assistant,
                content: "B".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Branch 2: A -> X
        let node_x_id = session
            .append_message_to(
                node_a_id.clone(),
                Message {
                    role: Role::Assistant,
                    content: "X".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await
            .unwrap();

        // Branch 3: A -> Y
        let node_y_id = session
            .append_message_to(
                node_a_id.clone(),
                Message {
                    role: Role::Assistant,
                    content: "Y".to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await
            .unwrap();

        // Should have 3 leaves
        let leaves = session.get_all_leaf_ids().await.unwrap();
        assert_eq!(leaves.len(), 3);
        assert!(leaves.contains(&node_b_id));
        assert!(leaves.contains(&node_x_id));
        assert!(leaves.contains(&node_y_id));

        // Get branches info
        let branches = session.get_branches().await.unwrap();
        assert_eq!(branches.len(), 3);

        // Only Y should be active
        let active_branches: Vec<_> = branches.iter().filter(|b| b.is_active).collect();
        assert_eq!(active_branches.len(), 1);
        assert_eq!(active_branches[0].leaf_id, node_y_id);
    }

    #[tokio::test]
    async fn test_metadata_operations() {
        let store = Arc::new(MemoryStore::new());
        let config = SessionConfig::default();
        let mut session = Session::new(store, config).await.unwrap();

        // Initially no metadata
        assert!(!session.has_metadata("test_key"));
        assert!(session.get_metadata::<String>("test_key").is_none());

        // Set string metadata
        session
            .set_metadata("test_key", &"test_value".to_string())
            .unwrap();
        assert!(session.has_metadata("test_key"));
        let value: String = session.get_metadata("test_key").unwrap();
        assert_eq!(value, "test_value");

        // Set complex metadata (like resolved config)
        use serde::{Deserialize, Serialize};
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestConfig {
            name: String,
            count: u32,
        }

        let test_config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        session.set_metadata("config", &test_config).unwrap();
        let loaded: TestConfig = session.get_metadata("config").unwrap();
        assert_eq!(loaded, test_config);

        // Remove metadata
        let removed = session.remove_metadata("test_key");
        assert!(removed.is_some());
        assert!(!session.has_metadata("test_key"));
        assert!(session.get_metadata::<String>("test_key").is_none());

        // Config should still be there
        assert!(session.has_metadata("config"));
    }
}
