//! Built-in tools for LLM agent interactions
//!
//! This module provides ready-to-use tools that can be used with any LLM provider.

pub mod bash;
pub mod editor_edit;

pub use bash::BashTool;
pub use editor_edit::EditorEditTool;

use crate::llm::ToolCall;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Boxed future type for async trait methods
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait for tools that can be registered with ToolRegistry
///
/// Implement this trait for custom tools to enable:
/// - Lazy loading of tool descriptions (saves tokens)
/// - Automatic registration with ToolRegistry
/// - Unified execution interface
pub trait ToolProvider: Send + Sync {
    /// Tool name (unique identifier)
    fn name(&self) -> &str;

    /// Brief description for token-efficient prompts
    ///
    /// This is what the LLM sees initially. Keep it short (under 100 chars).
    fn brief(&self) -> &str;

    /// Full description with usage details
    ///
    /// This is returned when LLM calls get-tool-detail.
    /// Include all usage notes, constraints, and examples here.
    fn full_description(&self) -> String;

    /// Parameter schema (JSON Schema format)
    fn parameters(&self) -> serde_json::Value;

    /// Execute the tool with the given call
    ///
    /// Returns Ok(output) on success, Err(message) on failure.
    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>>;
}

/// Returns all built-in tools for registration
///
/// Use this with `ToolRegistry::register_all_builtin()` to register
/// all available tools at once.
pub fn all_tools() -> Vec<Arc<dyn ToolProvider>> {
    vec![
        Arc::new(BashTool::new()),
        Arc::new(EditorEditTool::new()),
        // Add new built-in tools here
    ]
}
