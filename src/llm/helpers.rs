//! Helper functions for common LLM interaction patterns
//!
//! This module provides high-level helpers that wrap common patterns
//! like chat loops with tool execution and skill injection.

use super::rate_limit::{RateLimitConfig, RetryState};
use super::{LLMProvider, LoopStep, Message, Role, Tool, ToolCall, ToolResult};
use crate::skills::{
    build_skill_injections, parse_skill_references, render_skills_for_system_prompt,
    SkillReference, SkillsManager,
};
use crate::tools::InvokeSkillTool;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

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

/// Callback for skill injection warnings
pub type SkillWarningCallback = Box<dyn Fn(&str) + Send>;

/// Callback for when a skill is successfully injected
/// Parameters: (skill_name, skill_path)
pub type SkillInjectedCallback = Box<dyn Fn(&str, &str) + Send>;

/// Callback for rate limit retry events
/// Parameters: (attempt number, delay before retry, error message)
pub type RateLimitRetryCallback = Box<dyn Fn(u32, Duration, &str) + Send + Sync>;

/// Configuration for chat_loop_with_tools
pub struct ChatLoopConfig {
    /// Tool executors by tool name (used as fallback when tool not in registry)
    pub tool_executors: HashMap<String, ToolExecutor>,
    /// Tool registry for registered tools
    pub registry: Option<std::sync::Arc<super::registry::ToolRegistry>>,
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
    /// Optional callback for skill warnings
    pub on_skill_warning: Option<SkillWarningCallback>,
    /// Optional callback when a skill is injected
    pub on_skill_injected: Option<SkillInjectedCallback>,
    /// Maximum number of tool call rounds (default: 10)
    pub max_rounds: usize,
    /// Loop detection configuration (None to disable)
    pub loop_detection: Option<super::LoopDetectorConfig>,
    /// Skills manager for loading and caching skills
    pub skills_manager: Option<Arc<SkillsManager>>,
    /// Working directory for skill loading
    pub cwd: Option<PathBuf>,
    /// Enable automatic skill parsing from user messages (explicit /skill:name syntax)
    pub auto_parse_skills: bool,
    /// Enable implicit skill invocation (LLM decides based on task description)
    /// When enabled, skills are listed in system prompt and invoke_skill tool is added
    pub implicit_skills: bool,
    /// Rate limit configuration for retries
    pub rate_limit_config: Option<RateLimitConfig>,
    /// Callback for rate limit retry events
    pub on_rate_limit_retry: Option<Arc<RateLimitRetryCallback>>,
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
            on_skill_warning: None,
            on_skill_injected: None,
            max_rounds: 10,
            loop_detection: Some(super::LoopDetectorConfig::default()),
            skills_manager: None,
            cwd: None,
            auto_parse_skills: true,
            implicit_skills: false,
            rate_limit_config: Some(RateLimitConfig::default()),
            on_rate_limit_retry: None,
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
    pub fn with_registry(
        mut self,
        registry: std::sync::Arc<super::registry::ToolRegistry>,
    ) -> Self {
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

    /// Set skills manager for skill loading
    pub fn with_skills_manager(mut self, manager: Arc<SkillsManager>) -> Self {
        self.skills_manager = Some(manager);
        self
    }

    /// Set working directory for skill loading
    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Set skill warning callback
    pub fn on_skill_warning<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + 'static,
    {
        self.on_skill_warning = Some(Box::new(callback));
        self
    }

    /// Set skill injected callback
    ///
    /// Called when a skill is successfully loaded and injected into the conversation.
    /// Parameters: (skill_name, skill_path)
    pub fn on_skill_injected<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) + Send + 'static,
    {
        self.on_skill_injected = Some(Box::new(callback));
        self
    }

    /// Enable or disable automatic skill parsing from user messages
    pub fn with_auto_parse_skills(mut self, enabled: bool) -> Self {
        self.auto_parse_skills = enabled;
        self
    }

    /// Enable implicit skill invocation (LLM decides based on task description)
    ///
    /// When enabled:
    /// - Skills are listed in the system prompt with descriptions
    /// - The `invoke_skill` tool is automatically added
    /// - LLM can decide to use skills based on task matching
    pub fn with_implicit_skills(mut self, enabled: bool) -> Self {
        self.implicit_skills = enabled;
        self
    }

    /// Set rate limit configuration
    pub fn with_rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = Some(config);
        self
    }

    /// Disable rate limit retries
    pub fn without_rate_limit_retry(mut self) -> Self {
        self.rate_limit_config = None;
        self
    }

    /// Set rate limit retry callback
    pub fn on_rate_limit_retry<F>(mut self, callback: F) -> Self
    where
        F: Fn(u32, Duration, &str) + Send + Sync + 'static,
    {
        self.on_rate_limit_retry = Some(Arc::new(Box::new(callback)));
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
/// use aaagent::llm::*;
/// use aaagent::tools::BashTool;
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
    let mut full_content = String::new();
    let mut all_tool_calls = Vec::new();

    // Initialize loop detector if enabled
    let mut loop_detector = config
        .loop_detection
        .as_ref()
        .map(|cfg| super::LoopDetector::with_config(cfg.clone()));

    // Process skills if manager is configured
    let mut messages = messages;
    let mut tools = tools;
    let mut invoke_skill_tool: Option<InvokeSkillTool> = None;

    if let Some(ref skills_manager) = config.skills_manager {
        let cwd = config
            .cwd
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let outcome = skills_manager.skills_for_cwd(&cwd);

        // Handle implicit skills mode: inject skills into system prompt
        if config.implicit_skills && !outcome.skills.is_empty() {
            // Render skills section for system prompt
            if let Some(skills_section) = render_skills_for_system_prompt(&outcome.skills) {
                // Find or create system message
                let system_idx = messages.iter().position(|m| m.role == Role::System);

                if let Some(idx) = system_idx {
                    // Append to existing system message
                    let mut msg = messages[idx].clone();
                    msg.content = format!("{}\n\n{}", msg.content, skills_section);
                    messages[idx] = msg;
                } else {
                    // Insert new system message at the beginning
                    messages.insert(
                        0,
                        Message {
                            role: Role::System,
                            content: skills_section,
                            tool_call_id: None,
                            tool_calls: None,
                        },
                    );
                }
            }

            // Add invoke_skill tool
            let skill_tool = InvokeSkillTool::new(Arc::clone(skills_manager), cwd.clone());
            tools.push(InvokeSkillTool::definition());
            invoke_skill_tool = Some(skill_tool);
        }

        // Handle explicit skill references (auto_parse_skills mode)
        if config.auto_parse_skills {
            let mut skill_refs: Vec<SkillReference> = Vec::new();

            for msg in &messages {
                if msg.role == Role::User {
                    let refs = parse_skill_references(&msg.content);
                    skill_refs.extend(refs);
                }
            }

            // Build skill injections if we have references
            if !skill_refs.is_empty() {
                let injections = build_skill_injections(&skill_refs, Some(&outcome));

                // Report warnings
                for warning in &injections.warnings {
                    if let Some(ref callback) = config.on_skill_warning {
                        callback(warning);
                    }
                }

                // Inject skills as user messages before the conversation
                if !injections.is_empty() {
                    let mut new_messages =
                        Vec::with_capacity(messages.len() + injections.items.len());

                    // Insert skill injections at the beginning (after system message if present)
                    let insert_idx = messages
                        .iter()
                        .position(|m| m.role != Role::System)
                        .unwrap_or(0);

                    new_messages.extend(messages[..insert_idx].iter().cloned());

                    for injection in &injections.items {
                        // Notify callback about skill injection
                        if let Some(ref callback) = config.on_skill_injected {
                            callback(&injection.name, &injection.path);
                        }

                        new_messages.push(Message {
                            role: Role::User,
                            content: injection.to_xml(),
                            tool_call_id: None,
                            tool_calls: None,
                        });
                    }

                    new_messages.extend(messages[insert_idx..].iter().cloned());
                    messages = new_messages;
                }
            }
        }
    }

    // Start chat loop with rate limit retry
    let mut handle = start_chat_loop_with_retry(
        provider,
        messages,
        tools,
        config.rate_limit_config.as_ref(),
        config.on_rate_limit_retry.as_ref(),
    )
    .await?;

    let mut rounds = 0;

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

                // Check for loops and collect warnings
                let mut loop_warnings: HashMap<String, String> = HashMap::new();
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
                                            // Collect warning to prepend to tool result
                                            if let Some(warning) = detection.warning_message {
                                                loop_warnings.insert(call.id.clone(), warning);
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

                    // Check for invoke_skill tool first
                    let result = if call.name == "invoke_skill" {
                        if let Some(ref skill_tool) = invoke_skill_tool {
                            match skill_tool.execute_tool_call(call).await {
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
                                content:
                                    "invoke_skill tool not available (implicit_skills not enabled)"
                                        .to_string(),
                                is_error: true,
                            }
                        }
                    } else if let Some(ref registry) = config.registry {
                        // Try registry first, then fallback to executors
                        if let Some(result) = registry.execute(call).await {
                            result
                        } else if let Some(executor) = config.tool_executors.get(&call.name) {
                            // Tool not in registry, try executor
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
                        }
                    } else if let Some(executor) = config.tool_executors.get(&call.name) {
                        // No registry, use executor directly
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

                    // Prepend loop warning if present
                    let result = if let Some(warning) = loop_warnings.get(&call.id) {
                        ToolResult {
                            tool_call_id: result.tool_call_id,
                            content: format!("{}\n\n{}", warning, result.content),
                            is_error: result.is_error,
                        }
                    } else {
                        result
                    };

                    results.push(result);
                }

                // Notify callback with results
                if let Some(ref callback) = config.on_tool_results {
                    callback(&results);
                }

                // Submit results once
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

    unreachable!()
}

/// Inject skills into a message list.
///
/// This is a convenience function for manually injecting skills
/// without using the full chat loop.
///
/// # Arguments
/// * `messages` - The message list to inject into
/// Start a chat loop with rate limit retry logic.
async fn start_chat_loop_with_retry<P: LLMProvider>(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    rate_limit_config: Option<&RateLimitConfig>,
    on_retry: Option<&Arc<RateLimitRetryCallback>>,
) -> Result<super::ChatLoopHandle, super::ProviderError> {
    let config = match rate_limit_config {
        Some(cfg) => cfg.clone(),
        None => return provider.chat_loop(messages, Some(tools)).await,
    };

    let mut state = RetryState::new();

    loop {
        match provider
            .chat_loop(messages.clone(), Some(tools.clone()))
            .await
        {
            Ok(handle) => return Ok(handle),
            Err(e) => {
                let error_str = e.to_string();

                // Check if it's a rate limit error
                let is_rate_limit = error_str.contains("429")
                    || error_str.contains("rate_limit")
                    || error_str.contains("RESOURCE_EXHAUSTED")
                    || error_str.contains("Too Many Requests");

                if !is_rate_limit || !state.should_retry(&config) {
                    return Err(e);
                }

                // Parse rate limit info for better delay calculation
                state.rate_limit_info =
                    super::rate_limit::RateLimitInfo::from_gemini_error(&error_str)
                        .or_else(|| super::rate_limit::RateLimitInfo::from_openai_error(&error_str))
                        .or_else(|| {
                            super::rate_limit::RateLimitInfo::from_anthropic_error(&error_str)
                        });

                state.last_error = Some(error_str.clone());

                let delay = state.next_delay(&config);

                // Notify callback
                if let Some(callback) = on_retry {
                    callback(state.attempt + 1, delay, &error_str);
                }

                // Wait before retry
                tokio::time::sleep(delay).await;

                state.add_delay(delay);
                state.increment();
            }
        }
    }
}

/// * `skill_refs` - Skill references to inject
/// * `skills_manager` - The skills manager to use
/// * `cwd` - Working directory for skill loading
///
/// # Returns
/// A tuple of (modified messages, warnings)
pub fn inject_skills(
    messages: Vec<Message>,
    skill_refs: &[SkillReference],
    skills_manager: &SkillsManager,
    cwd: &std::path::Path,
) -> (Vec<Message>, Vec<String>) {
    if skill_refs.is_empty() {
        return (messages, vec![]);
    }

    let outcome = skills_manager.skills_for_cwd(cwd);
    let injections = build_skill_injections(skill_refs, Some(&outcome));

    if injections.is_empty() {
        return (messages, injections.warnings);
    }

    let mut new_messages = Vec::with_capacity(messages.len() + injections.items.len());

    // Insert skill injections at the beginning (after system message if present)
    let insert_idx = messages
        .iter()
        .position(|m| m.role != Role::System)
        .unwrap_or(0);

    new_messages.extend(messages[..insert_idx].iter().cloned());

    for injection in &injections.items {
        new_messages.push(Message {
            role: Role::User,
            content: injection.to_xml(),
            tool_call_id: None,
            tool_calls: None,
        });
    }

    new_messages.extend(messages[insert_idx..].iter().cloned());

    (new_messages, injections.warnings)
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

    #[test]
    fn test_config_with_skills() {
        let config = ChatLoopConfig::new()
            .with_cwd(PathBuf::from("/test"))
            .with_auto_parse_skills(false);

        assert_eq!(config.cwd, Some(PathBuf::from("/test")));
        assert!(!config.auto_parse_skills);
    }
}
