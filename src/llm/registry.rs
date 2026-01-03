//! Tool Registry for managing tool definitions
//!
//! Provides centralized tool management with support for:
//! - Registering tools with their providers
//! - Executing tool calls

use super::{Tool, ToolCall, ToolResult};
use crate::tools::ToolProvider;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry for managing tools
///
/// # Example
///
/// ```no_run
/// use km_tools::llm::ToolRegistry;
/// use km_tools::tools::BashTool;
///
/// // Register all built-in tools
/// let registry = ToolRegistry::new().register_all_builtin();
///
/// // Get tools to send to LLM
/// let tools = registry.get_tools_for_llm();
/// ```
pub struct ToolRegistry {
    /// All registered tools
    tools: HashMap<String, Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register<T: ToolProvider + 'static>(mut self, tool: T) -> Self {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
        self
    }

    /// Register all built-in tools from the tools module
    pub fn register_all_builtin(mut self) -> Self {
        for tool in crate::tools::all_tools() {
            let name = tool.name().to_string();
            self.tools.insert(name, tool);
        }
        self
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ToolProvider>> {
        self.tools.get(name)
    }

    /// Get all tool names
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Get tools to send to LLM (full definitions)
    pub fn get_tools_for_llm(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|provider| Tool {
                name: provider.name().to_string(),
                description: provider.full_description(),
                parameters: provider.parameters(),
                full_description: None,
            })
            .collect()
    }

    /// Execute a tool call
    ///
    /// Returns `Some(ToolResult)` if the tool was found and executed,
    /// `None` if the tool is not registered.
    pub async fn execute(&self, call: &ToolCall) -> Option<ToolResult> {
        let tool = self.tools.get(&call.name)?.clone();
        Some(match tool.execute(call).await {
            Ok(output) => ToolResult {
                tool_call_id: call.id.clone(),
                content: output,
                is_error: false,
            },
            Err(error) => ToolResult {
                tool_call_id: call.id.clone(),
                content: error,
                is_error: true,
            },
        })
    }

    /// Check if a tool is registered
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ToolRegistry::new();
        assert!(registry.tool_names().is_empty());
    }

    #[test]
    fn test_register_all_builtin() {
        let registry = ToolRegistry::new().register_all_builtin();
        assert!(registry.get("bash").is_some());
    }

    #[test]
    fn test_get_tools_for_llm() {
        let registry = ToolRegistry::new().register_all_builtin();
        let tools = registry.get_tools_for_llm();

        // Should have bash tool with full description
        let bash_tool = tools.iter().find(|t| t.name == "bash");
        assert!(bash_tool.is_some());
        assert!(bash_tool.unwrap().description.len() > 50);
    }

    #[test]
    fn test_execute_tool() {
        use crate::tools::BashTool;

        let registry = ToolRegistry::new().register(BashTool::new());

        let call = ToolCall {
            id: "test".to_string(),
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "echo hello"}),
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(registry.execute(&call));

        assert!(result.is_some());
        let result = result.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[test]
    fn test_execute_unknown_tool() {
        let registry = ToolRegistry::new();

        let call = ToolCall {
            id: "test".to_string(),
            name: "unknown".to_string(),
            arguments: serde_json::json!({}),
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(registry.execute(&call));

        assert!(result.is_none());
    }

    #[test]
    fn test_has_tool() {
        let registry = ToolRegistry::new().register_all_builtin();
        assert!(registry.has_tool("bash"));
        assert!(!registry.has_tool("nonexistent"));
    }
}
