//! Node data structures for the conversation tree

use serde::{Deserialize, Serialize};

use crate::llm::{Message, Role, ToolCall};

/// Node ID (ULID - time-ordered, sortable)
pub type NodeId = String;

/// Session ID (ULID - time-ordered, sortable)
pub type SessionId = String;

/// A node in the conversation tree.
///
/// **Immutability**: All fields except `flags` are immutable after insertion.
/// This guarantees append-only semantics and prevents history rewriting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    // Identity (IMMUTABLE)
    pub node_id: NodeId,
    pub session_id: SessionId,
    pub parent_id: Option<NodeId>, // null only for root

    // Type (IMMUTABLE)
    pub kind: NodeKind,
    pub role: Option<Role>, // nullable, required for Message kind

    // Content (IMMUTABLE)
    pub content_type: ContentType,
    pub content: String,

    // Metadata (IMMUTABLE except flags)
    pub created_at: i64,  // Unix timestamp
    pub seq: u32,         // ordering among siblings
    pub flags: NodeFlags, // MUTABLE (only field that can change)

    // Kind-specific data (IMMUTABLE)
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,

    // Pruning metadata (set by pruning operations, not part of initial data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruned_at: Option<i64>,

    // Future extension point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

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
    Base64, // Base64-encoded binary data (e.g., images for vision models)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeFlags {
    pub important: bool, // Never prune
    pub ephemeral: bool, // Can prune after time
    pub hidden: bool,    // Skip in context extraction
}

/// Checkpoint data stored as metadata, not as tree nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    /// Compacted summary of the conversation up to this node
    pub summary: String,

    /// When this checkpoint was created
    pub created_at: i64,

    /// Strategy used to create this checkpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>, // "manual" | "auto_count" | "auto_token_limit"

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

// Helper implementations
impl Node {
    /// Convert node to Message for LLM consumption
    pub fn to_message(&self) -> Message {
        Message {
            role: self.role.clone().unwrap_or(Role::User),
            content: self.content.clone(),
            tool_call_id: self.tool_call_id.clone(),
            tool_calls: self.tool_calls.clone(),
        }
    }

    /// Create a new root node
    pub fn new_root(session_id: SessionId) -> Self {
        Self {
            node_id: new_node_id(),
            session_id,
            parent_id: None,
            kind: NodeKind::Root,
            role: None,
            content_type: ContentType::Text,
            content: String::new(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        }
    }
}

/// Generate new ULID-based NodeId (time-ordered)
pub fn new_node_id() -> NodeId {
    ulid::Ulid::new().to_string()
}

/// Generate new ULID-based SessionId (time-ordered)
pub fn new_session_id() -> SessionId {
    ulid::Ulid::new().to_string()
}

/// Get current Unix timestamp in seconds
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Archived tool result for later retrieval via recall_tool_result
///
/// When tool results are compressed from context, the full content is archived
/// here so the LLM can retrieve it on-demand using the recall_tool_result tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedToolResult {
    /// The tool_call_id from the original ToolCall
    pub tool_call_id: String,

    /// Name of the tool that was called
    pub tool_name: String,

    /// Full content of the tool result
    pub full_content: String,

    /// Node ID of the tool result message
    pub node_id: NodeId,

    /// When this result was created
    pub created_at: i64,

    /// Size of the content in bytes
    pub content_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ulid_ordering() {
        let id1 = new_node_id();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let id2 = new_node_id();

        // Earlier ULIDs should sort before later ones
        assert!(id1 < id2);
    }

    #[test]
    fn test_root_node_creation() {
        let session_id = new_session_id();
        let root = Node::new_root(session_id.clone());

        assert_eq!(root.kind, NodeKind::Root);
        assert_eq!(root.session_id, session_id);
        assert!(root.parent_id.is_none());
        assert_eq!(root.seq, 0);
    }

    #[test]
    fn test_node_to_message() {
        let node = Node {
            node_id: new_node_id(),
            session_id: new_session_id(),
            parent_id: None,
            kind: NodeKind::Message,
            role: Some(Role::User),
            content_type: ContentType::Text,
            content: "Hello".to_string(),
            created_at: now(),
            seq: 0,
            flags: NodeFlags::default(),
            tool_call_id: None,
            tool_calls: None,
            pruned_at: None,
            metadata: None,
        };

        let msg = node.to_message();
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }
}
