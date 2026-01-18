//! Tree-based conversation history system
//!
//! This module provides a tree structure for storing conversation history,
//! supporting branching, replay, and checkpoint-based compaction.

pub mod compressor;
pub mod jsonl_store;
pub mod memory_store;
pub mod node;
pub mod session;
pub mod storage;
pub mod validator;

pub use compressor::{CompressionConfig, ContextCompressor};
pub use jsonl_store::JSONLStore;
pub use memory_store::MemoryStore;
pub use node::{
    ArchivedToolResult, CheckpointData, CheckpointStats, Node, NodeFlags, NodeId, NodeKind,
};
pub use session::{
    BranchInfo, CheckpointConfig, ContextOptimizationConfig, ContextStringStrategy, Session,
    SessionConfig, SessionId, SessionStats,
};
pub use storage::{NodeFilter, TreeStore};
pub use validator::MessageValidator;

// Re-export for convenience
pub use crate::llm::{Message, Role, ToolCall};
