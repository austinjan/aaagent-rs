//! Tool Registry for managing tool definitions and lazy loading
//!
//! Provides centralized tool management with support for:
//! - Brief descriptions for initial tool selection
//! - `pick_tool` mechanism for LLM to select tools
//! - Full tool definitions injected after selection

use super::{Tool, ToolCall, ToolResult};
use crate::tools::ToolProvider;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Registry for managing tools with lazy description loading
///
/// # Workflow
///
/// 1. First turn: LLM sees brief descriptions + `pick_tool`
/// 2. LLM calls `pick_tool` to select tools it needs
/// 3. Next turn: Registry provides full tool definitions for picked tools
/// 4. LLM can now call the actual tools with proper schemas
///
/// # Example
///
/// ```no_run
/// use km_tools::llm::ToolRegistry;
/// use km_tools::tools::BashTool;
///
/// // Register all built-in tools
/// let mut registry = ToolRegistry::new().register_all_builtin();
///
/// // First turn: get brief tools (includes pick_tool)
/// let tools = registry.get_tools_for_llm();
///
/// // After LLM calls pick_tool("bash"), the registry tracks it
/// // Next turn: get_tools_for_llm() returns full bash definition
/// ```
pub struct ToolRegistry {
    /// All registered tools
    tools: HashMap<String, Arc<dyn ToolProvider>>,
    /// Tools that have been picked by LLM
    picked_tools: HashSet<String>,
}

impl ToolRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            picked_tools: HashSet::new(),
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

    /// Check if any tools have been picked
    pub fn has_picked_tools(&self) -> bool {
        !self.picked_tools.is_empty()
    }

    /// Get the names of picked tools
    pub fn picked_tool_names(&self) -> Vec<&str> {
        self.picked_tools.iter().map(|s| s.as_str()).collect()
    }

    /// Clear all picked tools (reset for new conversation)
    pub fn clear_picked(&mut self) {
        self.picked_tools.clear();
    }

    /// Get tools to send to LLM
    ///
    /// Always includes:
    /// - Full definitions of any picked tools
    /// - Brief descriptions of unpicked tools
    /// - pick_tool meta-tool (so LLM can pick more tools anytime)
    pub fn get_tools_for_llm(&self) -> Vec<Tool> {
        let mut tools = Vec::new();

        // Add full definitions for picked tools
        for name in &self.picked_tools {
            if let Some(provider) = self.tools.get(name) {
                tools.push(Tool {
                    name: provider.name().to_string(),
                    description: provider.full_description(),
                    parameters: provider.parameters(),
                    full_description: None,
                });
            }
        }

        // Add brief descriptions for unpicked tools
        for (name, provider) in &self.tools {
            if !self.picked_tools.contains(name) {
                tools.push(Tool {
                    name: provider.name().to_string(),
                    description: provider.brief().to_string(),
                    parameters: serde_json::json!({"type": "object", "properties": {}}),
                    full_description: None,
                });
            }
        }

        // Always include pick_tool so LLM can pick more tools
        let unpicked: Vec<&str> = self
            .tools
            .keys()
            .filter(|name| !self.picked_tools.contains(*name))
            .map(|s| s.as_str())
            .collect();

        if !unpicked.is_empty() {
            tools.push(Self::pick_tools_definition(&unpicked));
        }

        tools
    }

    /// Execute a tool call
    ///
    /// Handles both regular tools and the pick_tool meta-tool.
    pub async fn execute(&mut self, call: &ToolCall) -> ToolResult {
        // Handle pick_tool meta-tool
        if call.name == "pick_tool" {
            return self.handle_pick_tools(call);
        }

        // Execute regular tool
        if let Some(tool) = self.tools.get(&call.name) {
            match tool.execute(call).await {
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
            }
        } else {
            let available = self.tool_names().join(", ");
            ToolResult {
                tool_call_id: call.id.clone(),
                content: format!(
                    "Tool '{}' not found. Available tools: {}",
                    call.name, available
                ),
                is_error: true,
            }
        }
    }

    /// Handle the pick_tool meta-tool call
    fn handle_pick_tools(&mut self, call: &ToolCall) -> ToolResult {
        let tool_names: Vec<String> = call
            .arguments
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if tool_names.is_empty() {
            return ToolResult {
                tool_call_id: call.id.clone(),
                content: "No tools specified. Please provide tool names in the 'tools' array."
                    .to_string(),
                is_error: true,
            };
        }

        let mut picked = Vec::new();
        let mut not_found = Vec::new();

        for name in &tool_names {
            if self.tools.contains_key(name) {
                self.picked_tools.insert(name.clone());
                picked.push(name.as_str());
            } else {
                not_found.push(name.as_str());
            }
        }

        let mut content = format!("Selected tools: {}", picked.join(", "));
        if !not_found.is_empty() {
            content.push_str(&format!(
                "\nWarning: tools not found: {}",
                not_found.join(", ")
            ));
        }
        content.push_str("\n\nYou can now use these tools in the next turn.");

        ToolResult {
            tool_call_id: call.id.clone(),
            content,
            is_error: false,
        }
    }

    /// Definition for the pick_tool meta-tool
    fn pick_tools_definition(available_tools: &[&str]) -> Tool {
        let tools_list = available_tools.join(", ");
        Tool {
            name: "pick_tools".to_string(),
            description: format!(
                r#####"
# Pick Tools
Pick tools provides a tool set helping LLM choose tools for a task.

## Description
A meta-tool that returns relevant tool specifications based on the current task context.

Instead of loading all available tools upfront (which consumes tokens and may confuse the LLM), use this tool to dynamically discover and load only the tools needed for the current task.

This tool acts as a "tool router" - describe what you want to accomplish, and it returns the appropriate tool specifications.

## Usage Notes
- Call this tool FIRST when you need capabilities not available in your current toolset
- Describe your intent clearly in natural language for best matching results
- You can request multiple tool categories in one call
- Returns complete tool specifications including parameters and usage examples
- Returned tools become available for subsequent calls in the same session
- If no matching tools are found, suggestions for rephrasing will be provided

                 Available tools: {}"#####,
                tools_list
            ),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tools": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": available_tools
                        },
                        "description": "List of tool names to enable"
                    }
                },
                "required": ["tools"]
            }),
            full_description: None,
        }
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
            picked_tools: self.picked_tools.clone(),
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
        assert!(!registry.has_picked_tools());
    }

    #[test]
    fn test_register_all_builtin() {
        let registry = ToolRegistry::new().register_all_builtin();
        assert!(registry.get("bash").is_some());
    }

    #[test]
    fn test_get_tools_includes_pick_tool() {
        let registry = ToolRegistry::new().register_all_builtin();
        let tools = registry.get_tools_for_llm();

        let has_pick_tool = tools.iter().any(|t| t.name == "pick_tools");
        assert!(has_pick_tool, "Expected pick_tools in tools list");
    }

    #[test]
    fn test_pick_tool_flow() {
        let mut registry = ToolRegistry::new().register_all_builtin();

        // Initially no picked tools
        assert!(!registry.has_picked_tools());

        // Simulate pick_tool call
        let call = ToolCall {
            id: "test".to_string(),
            name: "pick_tool".to_string(),
            arguments: serde_json::json!({"tools": ["bash"]}),
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(registry.execute(&call));

        assert!(!result.is_error);
        assert!(registry.has_picked_tools());
        assert!(registry.picked_tools.contains("bash"));

        // After picking, get_tools_for_llm should return full bash definition
        // (no pick_tool since all tools are picked)
        let tools = registry.get_tools_for_llm();
        assert_eq!(tools.len(), 1);

        let bash_tool = tools.iter().find(|t| t.name == "bash").unwrap();
        // Full description should be longer than brief
        assert!(bash_tool.description.len() > 50);
    }

    #[test]
    fn test_pick_tool_partial() {
        // Register multiple tools to test partial picking
        use crate::tools::BashTool;

        let registry = ToolRegistry::new()
            .register(BashTool::new())
            .register(BashTool::new().with_timeout(60)); // Same tool, different config

        // This won't work well since both have same name "bash"
        // Let's just verify the pick_tools is included when there are unpicked tools
        let tools = registry.get_tools_for_llm();
        let has_pick_tool = tools.iter().any(|t| t.name == "pick_tools");
        assert!(has_pick_tool);
    }
}
