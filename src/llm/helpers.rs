//! Helper functions for common LLM interaction patterns
//!
//! This module provides high-level helpers that wrap common patterns
//! like chat loops with tool execution.

use super::registry::ToolRegistry;
use super::{LLMProvider, LoopStep, Message, Role, Tool, ToolCall, ToolResult};
use crate::log;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// Tool executor function type
///
/// Takes a ToolCall and returns a Future that resolves to a Result<String, String>
/// - Ok(String) for successful execution with output
/// - Err(String) for execution errors
pub type ToolExecutor =
    Box<dyn Fn(ToolCall) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send>;

/// Event callback for streaming content
///
/// Called when the LLM generates text content
pub type ContentCallback = Box<dyn Fn(&str) + Send>;

/// Callback for when tool calls are requested
///
/// Called before tool execution, allows for logging/UI updates
pub type ToolCallCallback = Box<dyn Fn(&[ToolCall]) + Send>;

/// Callback for when tool results are ready
///
/// Called after tool execution, before submitting to LLM
pub type ToolResultCallback = Box<dyn Fn(&[ToolResult]) + Send>;

/// Configuration for chat_loop_with_tools
pub struct ChatLoopConfig {
    /// Tool executors by tool name (legacy, used when registry is None)
    pub tool_executors: HashMap<String, ToolExecutor>,
    /// Tool registry for lazy loading (preferred over tool_executors)
    pub registry: Option<ToolRegistry>,
    /// Optional callback for streaming content
    pub on_content: Option<ContentCallback>,
    /// Optional callback when tool calls are requested
    pub on_tool_calls: Option<ToolCallCallback>,
    /// Optional callback when tool results are ready
    pub on_tool_results: Option<ToolResultCallback>,
    /// Optional callback for thinking content (Claude, o1, etc.)
    pub on_thinking: Option<ContentCallback>,
    /// Maximum number of tool call rounds (default: 10)
    pub max_rounds: usize,
}

impl ChatLoopConfig {
    /// Create a new configuration
    pub fn new() -> Self {
        Self {
            tool_executors: HashMap::new(),
            registry: None,
            on_content: None,
            on_tool_calls: None,
            on_tool_results: None,
            on_thinking: None,
            max_rounds: 10,
        }
    }

    /// Use a ToolRegistry for lazy tool loading
    ///
    /// When set, the registry handles all tool execution automatically.
    /// LLM first sees brief descriptions + pick_tool, then full definitions after picking.
    pub fn with_registry(mut self, registry: ToolRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Register a tool executor (legacy method)
    ///
    /// Prefer `with_registry()` for new code.
    pub fn with_tool<F, Fut>(mut self, name: impl Into<String>, executor: F) -> Self
    where
        F: Fn(ToolCall) -> Fut + Send + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        self.tool_executors
            .insert(name.into(), Box::new(move |call| Box::pin(executor(call))));
        self
    }

    /// Set content callback
    pub fn on_content<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + 'static,
    {
        self.on_content = Some(Box::new(callback));
        self
    }

    /// Set tool call callback
    pub fn on_tool_calls<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[ToolCall]) + Send + 'static,
    {
        self.on_tool_calls = Some(Box::new(callback));
        self
    }

    /// Set tool result callback
    pub fn on_tool_results<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[ToolResult]) + Send + 'static,
    {
        self.on_tool_results = Some(Box::new(callback));
        self
    }

    /// Set thinking callback
    pub fn on_thinking<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + 'static,
    {
        self.on_thinking = Some(Box::new(callback));
        self
    }

    /// Set maximum rounds
    pub fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }
}

impl Default for ChatLoopConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from chat_loop_with_tools
#[derive(Debug, Clone)]
pub struct ChatLoopResponse {
    /// Final content from the LLM
    pub content: String,
    /// Total token usage
    pub usage: super::TokenUsage,
    /// All tool calls made during the conversation
    pub all_tool_calls: Vec<ToolCall>,
    /// Number of rounds executed
    pub rounds: usize,
}

/// High-level helper for running a chat loop with automatic tool execution
///
/// This function handles the entire chat loop lifecycle:
/// - Streams content to callbacks
/// - Automatically executes tools using registered executors or registry
/// - Handles multiple rounds of tool calling
/// - Returns the final result
///
/// # Example (with registry - recommended)
///
/// ```no_run
/// use km_tools::llm::*;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let provider = OpenAIProvider::create("gpt-4".to_string(), "key".to_string())?;
///     let registry = ToolRegistry::new().register_all_builtin();
///
///     let config = ChatLoopConfig::new()
///         .with_registry(registry)
///         .on_content(|text| print!("{}", text));
///
///     let response = chat_with_registry(
///         &provider,
///         vec![Message {
///             role: Role::User,
///             content: "List files in current directory".to_string(),
///             tool_call_id: None,
///             tool_calls: None,
///         }],
///         config
///     ).await?;
///
///     println!("Done! Used {} tokens", response.usage.total());
///     Ok(())
/// }
/// ```
pub async fn chat_loop_with_tools<P: LLMProvider>(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    mut config: ChatLoopConfig,
) -> Result<ChatLoopResponse, super::ProviderError> {
    log("Start chat_loop_with_tools");

    for (idx, msg) in messages.iter().enumerate() {
        log(format!("  [input:{}]  {}", idx + 1, msg));
    }

    // Determine which tools to send to LLM
    let tools_for_llm = if let Some(ref registry) = config.registry {
        registry.get_tools_for_llm()
    } else {
        tools.clone()
    };

    let mut current_messages = messages;
    let mut handle = provider
        .chat_loop(current_messages.clone(), Some(tools_for_llm))
        .await?;

    let mut full_content = String::new();
    let mut all_tool_calls = Vec::new();
    let mut rounds = 0;
    let mut total_usage = super::TokenUsage::default();

    loop {
        let event_result = handle.next().await;

        let event = match event_result {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err(e),
            None => {
                log("[error] chat_loop ended unexpectedly");
                return Err(super::ProviderError::ApiError(
                    "Chat loop ended unexpectedly".to_string(),
                ));
            }
        };

        match event {
            LoopStep::Thinking(thought) => {
                log(format!("[thinking] {}", truncate_for_log(&thought)));
                if let Some(ref callback) = config.on_thinking {
                    callback(&thought);
                }
            }
            LoopStep::Content(text) => {
                log(format!("[content] {}", truncate_for_log(&text)));
                full_content.push_str(&text);
                if let Some(ref callback) = config.on_content {
                    callback(&text);
                }
            }
            LoopStep::ToolCallsRequested {
                tool_calls,
                content,
            } => {
                rounds += 1;
                log(format!(
                    "[tool_calls] round={} content={}",
                    rounds,
                    truncate_for_log(&content)
                ));
                for call in &tool_calls {
                    log(format!("  [call] {} -> {}", call.id, call.name));
                }

                if rounds > config.max_rounds {
                    log(format!(
                        "[error] max rounds exceeded ({})",
                        config.max_rounds
                    ));
                    return Err(super::ProviderError::ApiError(format!(
                        "Maximum rounds ({}) exceeded",
                        config.max_rounds
                    )));
                }

                if !content.is_empty() {
                    full_content.push_str(&content);
                }

                if let Some(ref callback) = config.on_tool_calls {
                    callback(&tool_calls);
                }

                // Check if pick_tool was called
                let has_pick_tool = tool_calls.iter().any(|c| c.name == "pick_tool");

                // Execute tools
                let mut results = Vec::new();
                for call in &tool_calls {
                    all_tool_calls.push(call.clone());

                    let result = if let Some(ref mut registry) = config.registry {
                        log(format!("[exec:registry] {} ({})", call.id, call.name));
                        registry.execute(call).await
                    } else if let Some(executor) = config.tool_executors.get(&call.name) {
                        log(format!("[exec] {} ({})", call.id, call.name));
                        match executor(call.clone()).await {
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
                        log(format!(
                            "[error] missing executor: {} ({})",
                            call.id, call.name
                        ));
                        ToolResult {
                            tool_call_id: call.id.clone(),
                            content: format!("Tool '{}' not registered", call.name),
                            is_error: true,
                        }
                    };

                    results.push(result);
                }

                if let Some(ref callback) = config.on_tool_results {
                    callback(&results);
                }

                for result in &results {
                    let tag = if result.is_error {
                        "[result:error]"
                    } else {
                        "[result]"
                    };
                    log(format!(
                        "{} {} {}",
                        tag,
                        result.tool_call_id,
                        truncate_for_log(&result.content)
                    ));
                }

                // If pick_tool was called, restart chat_loop with new tools
                if has_pick_tool && config.registry.is_some() {
                    log("[pick_tool] restarting chat_loop with picked tools");

                    // Build updated message history
                    // Add assistant message with tool calls
                    current_messages.push(Message {
                        role: Role::Assistant,
                        content: content.clone(),
                        tool_call_id: None,
                        tool_calls: Some(tool_calls.clone()),
                    });

                    // Add tool results
                    for result in &results {
                        current_messages.push(Message {
                            role: Role::Tool,
                            content: result.content.clone(),
                            tool_call_id: Some(result.tool_call_id.clone()),
                            tool_calls: None,
                        });
                    }

                    // Get new tools (now with full definitions for picked tools)
                    let new_tools = config.registry.as_ref().unwrap().get_tools_for_llm();
                    log(format!(
                        "[pick_tool] new tools: {:?}",
                        new_tools.iter().map(|t| &t.name).collect::<Vec<_>>()
                    ));

                    // Start new chat_loop with updated history and tools
                    handle = provider
                        .chat_loop(current_messages.clone(), Some(new_tools))
                        .await?;
                    continue;
                }

                // Normal flow: submit results and continue
                handle.submit_tool_results(results)?;
            }
            LoopStep::ToolResultsReceived { .. } => {
                log("[results_received]");
            }
            LoopStep::Done {
                content,
                total_usage: usage,
                finish_reason,
                ..
            } => {
                log(format!(
                    "[done] reason={:?} in={} out={}",
                    finish_reason, usage.input_tokens, usage.output_tokens
                ));

                total_usage.input_tokens += usage.input_tokens;
                total_usage.output_tokens += usage.output_tokens;
                total_usage.cached_tokens += usage.cached_tokens;

                if !content.is_empty() && content != full_content {
                    full_content = content;
                }

                return Ok(ChatLoopResponse {
                    content: full_content,
                    usage: total_usage,
                    all_tool_calls,
                    rounds,
                });
            }
        }
    }
}

/// Convenience function for chat loop with ToolRegistry
///
/// This is the recommended way to use the chat loop with lazy tool loading.
/// Tools are registered once, and the registry handles both description
/// serving and execution.
///
/// # Workflow
///
/// 1. LLM sees brief tool descriptions + `pick_tool`
/// 2. LLM calls `pick_tool` to select needed tools
/// 3. Chat restarts with full definitions of picked tools
/// 4. LLM can now use the actual tools
///
/// # Example
///
/// ```no_run
/// use km_tools::llm::*;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let provider = OpenAIProvider::create("gpt-4".to_string(), "key".to_string())?;
///
///     // Register all built-in tools
///     let registry = ToolRegistry::new().register_all_builtin();
///
///     let config = ChatLoopConfig::new()
///         .with_registry(registry)
///         .on_content(|text| print!("{}", text));
///
///     let messages = vec![Message {
///         role: Role::User,
///         content: "List files".to_string(),
///         tool_call_id: None,
///         tool_calls: None,
///     }];
///     let response = chat_with_registry(&provider, messages, config).await?;
///     Ok(())
/// }
/// ```
pub async fn chat_with_registry<P: LLMProvider>(
    provider: &P,
    messages: Vec<Message>,
    config: ChatLoopConfig,
) -> Result<ChatLoopResponse, super::ProviderError> {
    if config.registry.is_none() {
        return Err(super::ProviderError::ConfigError(
            "chat_with_registry requires config.with_registry() to be set".to_string(),
        ));
    }
    chat_loop_with_tools(provider, messages, vec![], config).await
}

fn truncate_for_log(text: &str) -> String {
    const LIMIT: usize = 120;
    let mut result = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= LIMIT {
            result.push_str("...");
            return result;
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = ChatLoopConfig::new()
            .with_tool("test", |_call| async { Ok("result".to_string()) })
            .with_max_rounds(5);

        assert_eq!(config.max_rounds, 5);
        assert_eq!(config.tool_executors.len(), 1);
        assert!(config.tool_executors.contains_key("test"));
    }
}
