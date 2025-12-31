//! Helper functions for common LLM interaction patterns
//!
//! This module provides high-level helpers that wrap common patterns
//! like chat loops with tool execution.

use super::{LLMProvider, LoopStep, Message, Tool, ToolCall, ToolResult};
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

/// Callback for when a loop is detected
///
/// Called when the loop detector identifies a loop
/// Return true to continue, false to terminate
pub type LoopDetectionCallback = Box<dyn Fn(&super::LoopDetection) -> bool + Send>;

/// Configuration for chat_loop_with_tools
pub struct ChatLoopConfig {
    /// Tool executors by tool name (legacy, used when registry is None)
    pub tool_executors: HashMap<String, ToolExecutor>,
    /// Tool registry for lazy loading (preferred over tool_executors)
    pub registry: Option<super::registry::ToolRegistry>,
    /// Optional callback for streaming content
    pub on_content: Option<ContentCallback>,
    /// Optional callback when tool calls are requested
    pub on_tool_calls: Option<ToolCallCallback>,
    /// Optional callback when tool results are ready
    pub on_tool_results: Option<ToolResultCallback>,
    /// Optional callback for thinking content (Claude, o1, etc.)
    pub on_thinking: Option<ContentCallback>,
    /// Optional callback when a loop is detected
    pub on_loop_detected: Option<LoopDetectionCallback>,
    /// Maximum number of tool call rounds (default: 10)
    pub max_rounds: usize,
    /// Loop detection configuration (None to disable)
    pub loop_detection: Option<super::LoopDetectorConfig>,
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
            on_loop_detected: None,
            max_rounds: 10,
            loop_detection: Some(super::LoopDetectorConfig::default()),
        }
    }

    /// Register a tool executor
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

    /// Set loop detection callback
    pub fn on_loop_detected<F>(mut self, callback: F) -> Self
    where
        F: Fn(&super::LoopDetection) -> bool + Send + 'static,
    {
        self.on_loop_detected = Some(Box::new(callback));
        self
    }

    /// Set tool registry
    pub fn with_registry(mut self, registry: super::registry::ToolRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Set maximum rounds
    pub fn with_max_rounds(mut self, max_rounds: usize) -> Self {
        self.max_rounds = max_rounds;
        self
    }

    /// Set loop detection configuration
    pub fn with_loop_detection(mut self, config: super::LoopDetectorConfig) -> Self {
        self.loop_detection = Some(config);
        self
    }

    /// Disable loop detection
    pub fn without_loop_detection(mut self) -> Self {
        self.loop_detection = None;
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
/// - Automatically executes tools using registered executors
/// - Handles multiple rounds of tool calling
/// - Returns the final result
///
/// # Example
///
/// ```no_run
/// use km_tools::llm::*;
/// use km_tools::tools::BashTool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// #     let api_key = std::env::var("OPENAI_API_KEY")?;
///     let provider = OpenAIProvider::create("gpt-5-nano".to_string(), api_key)?;
///     let bash_tool = BashTool::new();
///     let tool_def = bash_tool.as_tool();
///
///     let config = ChatLoopConfig::new()
///         .with_tool("bash", {
///             let bash_tool = bash_tool.clone();
///             move |call| {
///                 let bash_tool = bash_tool.clone();
///                 async move { bash_tool.execute(&call).await }
///             }
///         })
///         .on_content(|text| print!("{}", text));
///
///     let response = chat_loop_with_tools(
///         &provider,
///         vec![Message {
///             role: Role::User,
///             content: "List files in current directory".to_string(),
///             tool_call_id: None,
///             tool_calls: None,
///         }],
///         vec![tool_def],
///         config
///     ).await?;
///
///     println!("Done! Used {} tokens", response.usage.total());
/// #     Ok(())
/// # }
/// ```
pub async fn chat_loop_with_tools<P: LLMProvider>(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    config: ChatLoopConfig,
) -> Result<ChatLoopResponse, super::ProviderError> {
    let mut handle = provider.chat_loop(messages, Some(tools)).await?;

    let mut full_content = String::new();
    let mut all_tool_calls = Vec::new();
    let mut rounds = 0;

    // Initialize loop detector if enabled
    let mut loop_detector = config
        .loop_detection
        .as_ref()
        .map(|cfg| super::LoopDetector::with_config(cfg.clone()));

    while let Some(event_result) = handle.next().await {
        let event = event_result?;

        match event {
            LoopStep::Thinking(thought) => {
                if let Some(ref callback) = config.on_thinking {
                    callback(&thought);
                }
            }
            LoopStep::Content(text) => {
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

                if rounds > config.max_rounds {
                    return Err(super::ProviderError::ApiError(format!(
                        "Maximum rounds ({}) exceeded",
                        config.max_rounds
                    )));
                }

                // Add any content before tool calls
                if !content.is_empty() {
                    full_content.push_str(&content);
                }

                // Notify callback
                if let Some(ref callback) = config.on_tool_calls {
                    callback(&tool_calls);
                }

                // Check for loops before executing tools
                if let Some(ref mut detector) = loop_detector {
                    for call in &tool_calls {
                        if let Some(detection) = detector.check(call) {
                            // Call user callback if provided
                            let should_continue =
                                if let Some(ref callback) = config.on_loop_detected {
                                    callback(&detection)
                                } else {
                                    // Default behavior based on action
                                    match detection.action {
                                        super::LoopAction::Continue => true,
                                        super::LoopAction::Warn => {
                                            // Inject warning message
                                            if let Some(warning) = detection.warning_message {
                                                handle.submit_tool_results(vec![ToolResult {
                                                    tool_call_id: call.id.clone(),
                                                    content: warning,
                                                    is_error: false,
                                                }])?;
                                            }
                                            true
                                        }
                                        super::LoopAction::Terminate => false,
                                    }
                                };

                            if !should_continue {
                                // Clear detector state and return error
                                detector.clear();
                                return Err(super::ProviderError::ApiError(format!(
                                    "Loop detected: {}",
                                    detection.suggestion
                                )));
                            }
                        }
                    }
                }

                // Execute tools
                let mut results = Vec::new();
                for call in &tool_calls {
                    all_tool_calls.push(call.clone());

                    let result = if let Some(executor) = config.tool_executors.get(&call.name) {
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
                        ToolResult {
                            tool_call_id: call.id.clone(),
                            content: format!("Tool '{}' not registered", call.name),
                            is_error: true,
                        }
                    };

                    results.push(result);
                }

                // Notify callback with results
                if let Some(ref callback) = config.on_tool_results {
                    callback(&results);
                }

                // Submit results
                handle.submit_tool_results(results)?;
            }
            LoopStep::ToolResultsReceived { .. } => {
                // Just continue
            }
            LoopStep::Done {
                content,
                total_usage,
                ..
            } => {
                // Update final content if provided
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

    Err(super::ProviderError::ApiError(
        "Chat loop ended unexpectedly".to_string(),
    ))
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
