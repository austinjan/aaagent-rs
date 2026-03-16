use anyhow::Result;
use async_trait::async_trait;

use super::node::{Node, NodeFlags, NodeId, SessionId};
use super::session::Session;
use crate::llm::Role;

/// Filter for querying nodes
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    pub kinds: Option<Vec<super::node::NodeKind>>,
    pub roles: Option<Vec<Role>>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub flags: Option<NodeFlags>,
    pub content_search: Option<String>,
}

/// TreeStore provides append-only storage for conversation tree nodes.
///
/// **Immutability Guarantee**: Nodes are immutable after insertion.
/// Core fields (node_id, parent_id, content, etc.) CANNOT be modified.
/// Only metadata (flags, pruning markers) may be updated.
#[async_trait]
pub trait TreeStore: Send + Sync {
    // Node operations (immutable)

    /// Insert a new node into the store
    async fn insert_node(&self, node: Node) -> Result<NodeId>;

    /// Get a node by its ID
    async fn get_node(&self, node_id: NodeId) -> Result<Option<Node>>;

    // Metadata updates (ONLY these fields allowed):

    /// Update only the flags of a node
    async fn update_node_flags(&self, node_id: NodeId, flags: NodeFlags) -> Result<()>;

    /// Mark a node as pruned (sets pruned_at timestamp)
    async fn mark_node_pruned(&self, node_id: NodeId, pruned_at: i64) -> Result<()>;

    // Deletion (for pruning only, validates safety)

    /// Delete a node permanently (use with caution, for pruning only)
    async fn delete_node(&self, node_id: NodeId) -> Result<()>;

    // Query operations (use via Session, not directly)

    /// Get all children of a node
    async fn get_children(&self, node_id: NodeId) -> Result<Vec<Node>>;

    /// **Internal use only** - Do NOT call directly from application code.
    /// Use `Session::get_context()` instead, which handles checkpoints correctly.
    ///
    /// This method blindly returns all nodes to root without stopping at checkpoints.
    async fn get_path_to_root_internal(&self, node_id: NodeId) -> Result<Vec<Node>>;

    /// Find nodes matching the filter
    async fn find_nodes(&self, session_id: SessionId, filter: NodeFilter) -> Result<Vec<Node>>;

    // Session operations

    /// Create a new session
    async fn create_session(&self, session: Session) -> Result<SessionId>;

    /// Get a session by its ID
    async fn get_session(&self, session_id: SessionId) -> Result<Option<Session>>;

    /// Update an existing session
    async fn update_session(&self, session: &Session) -> Result<()>;

    /// List all sessions
    async fn list_sessions(&self) -> Result<Vec<Session>>;

    /// Archive a session (sets archived flag to true)
    async fn archive_session(&self, session_id: SessionId) -> Result<()>;

    /// Delete a session and all its data permanently
    async fn delete_session(&self, session_id: SessionId) -> Result<()>;

    // Batch operations

    /// Get multiple nodes in a single operation
    async fn get_nodes_batch(&self, node_ids: Vec<NodeId>) -> Result<Vec<Node>>;

    /// Insert multiple nodes in a single transaction
    async fn insert_nodes_batch(&self, nodes: Vec<Node>) -> Result<Vec<NodeId>>;

    // Session-scoped operations (optimized for single-session queries)

    /// Get a node within a specific session (avoids global scan)
    ///
    /// This is more efficient than `get_node()` for stores that partition by session.
    /// For stores without partitioning (like MemoryStore), this may be equivalent.
    async fn get_node_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Option<Node>>;

    /// Get multiple nodes within a specific session (avoids global scan)
    ///
    /// More efficient than `get_nodes_batch()` when all nodes belong to the same session.
    async fn get_nodes_batch_in_session(
        &self,
        session_id: SessionId,
        node_ids: Vec<NodeId>,
    ) -> Result<Vec<Node>>;

    /// Get children of a node within a specific session (avoids global scan)
    ///
    /// More efficient than `get_children()` for partitioned stores.
    async fn get_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<Vec<Node>>;

    /// Count children of a node within a specific session
    ///
    /// More efficient than loading all children when you only need the count.
    async fn count_children_in_session(
        &self,
        session_id: SessionId,
        node_id: NodeId,
    ) -> Result<usize>;
}
