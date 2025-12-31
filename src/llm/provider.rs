use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

// ============================================================================
// Core Trait
// ============================================================================

/// Unified interface for LLM providers (OpenAI, Anthropic, Gemini)
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Create a new provider instance with model and API key
    fn create(model: String, api_key: String) -> Result<Self, ProviderError>
    where
        Self: Sized;

    /// Get current provider state (token usage, request count, etc.)
    /// Thread-safe: can be called while streaming is active
    fn state(&self) -> ProviderState;

    /// Get a copy of the current configuration
    fn config(&self) -> ProviderConfig;

    /// Update configuration using a closure
    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig));

    /// Simple chat completion (single prompt -> response)
    /// Returns a stream of chunks for streaming responses
    /// Uses &self (not &mut) for concurrent access
    async fn chat(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;

    /// Advanced chat loop with conversation history and tool calling
    /// Returns a ChatLoopHandle for bidirectional communication
    async fn chat_loop(
        &self,
        history: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError>;

    /// Enable prompt caching for expensive prompts
    fn prompt_cache(&mut self, cache_prompt: String) -> Result<(), ProviderError>;

    /// Compact conversation history to reduce token usage
    ///
    /// For long-running conversations, this compresses the history while preserving
    /// the model's understanding of the conversation context.
    ///
    /// Different providers implement this differently:
    /// - OpenAI: Uses the `/responses/compact` endpoint (Responses API)
    /// - Anthropic: May use context editing or summarization
    /// - Others: May use LLM-based summarization or simple truncation
    ///
    /// Returns the compacted history that can be used in subsequent chat_loop calls
    async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;

    /// Get the current conversation history
    ///
    /// Returns the accumulated conversation history from the last chat_loop call.
    /// This includes all messages (user, assistant, tool calls, tool results) that
    /// have been processed, after any automatic pruning has been applied.
    ///
    /// Note: History is only tracked during chat_loop. Simple chat() calls don't
    /// maintain history state.
    fn get_history(&self) -> Vec<Message>;
}

// ============================================================================
// State and Configuration
// ============================================================================

/// Provider state tracking usage statistics
#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    /// Total input tokens used
    pub input_tokens: u64,

    /// Total output tokens used
    pub output_tokens: u64,

    /// Total cached tokens (for providers supporting prompt caching)
    pub cached_tokens: u64,

    /// Number of API requests made
    pub request_count: u64,

    /// Last request timestamp
    pub last_request_time: Option<std::time::SystemTime>,

    /// Provider-specific metadata
    pub metadata: HashMap<String, String>,

    /// Total conversation loop turns
    pub conversation_turns: u32,
}

/// Configuration options for generation
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Temperature (0.0 - 1.0+)
    pub temperature: f32,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Top-p sampling
    pub top_p: Option<f32>,

    /// Top-k sampling (for supported providers)
    pub top_k: Option<u32>,

    /// Enable thinking/reasoning mode (for supported providers like Claude)
    pub enable_reasoning: bool,

    /// System prompt
    pub system_prompt: Option<String>,

    /// Stop sequences
    pub stop_sequences: Vec<String>,

    /// Provider-specific options
    pub extra_options: HashMap<String, serde_json::Value>,

    /// Maximum number of tool call/result turns to keep in history (None = unlimited)
    /// One turn = one assistant message with tool calls + corresponding tool result messages
    /// When limit is reached, oldest tool turns are removed
    /// Default: 3 turns
    pub max_tool_turns: Option<usize>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 40960, // Increased from 4096 to support longer responses
            top_p: None,
            top_k: None,
            enable_reasoning: false,
            system_prompt: None,
            stop_sequences: Vec::new(),
            extra_options: HashMap::new(),
            max_tool_turns: Some(3), // Keep last 3 tool turns by default
        }
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// Message role in conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,

    /// Tool call ID (for tool responses)
    pub tool_call_id: Option<String>,

    /// Tool calls made by assistant
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.role {
            Role::System => write!(f, "System: {}", self.content),
            Role::User => write!(f, "User: {}", self.content),
            Role::Assistant => write!(f, "Assistant: {}", self.content),
            Role::Tool => write!(f, "Tool: {}", self.content),
        }
    }
}

/// Tool call made by the LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

// ============================================================================
// Tool Definition and Results
// ============================================================================

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema

    /// Full detailed description (not serialized to LLM)
    ///
    /// When set, `description` is used as the brief description,
    /// and this field contains the full usage details.
    /// Use `get_full_description()` to retrieve the appropriate description.
    #[serde(skip_serializing, default)]
    pub full_description: Option<String>,
}

impl Tool {
    /// Create a new tool with brief and full descriptions
    pub fn new(
        name: impl Into<String>,
        brief: impl Into<String>,
        full_description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: brief.into(),
            parameters,
            full_description: Some(full_description.into()),
        }
    }

    /// Get the full description (falls back to brief if not set)
    pub fn get_full_description(&self) -> &str {
        self.full_description
            .as_deref()
            .unwrap_or(&self.description)
    }

    /// Create a brief version for sending to LLM (strips full_description)
    pub fn as_brief(&self) -> Tool {
        Tool {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: self.parameters.clone(),
            full_description: None,
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Must match the tool_call_id from ToolCall
    pub tool_call_id: String,

    /// Result content (can be string, JSON, or error message)
    pub content: String,

    /// Whether the tool execution was successful
    pub is_error: bool,
}

/// Internal: submission of tool results via channel
#[derive(Debug)]
pub(crate) struct ToolResultSubmission {
    pub(crate) results: Vec<ToolResult>,
}

// ============================================================================
// Helper: Tool Call Assembler
// ============================================================================

/// Helper for assembling parallel tool calls from deltas
#[derive(Debug, Default)]
pub struct ToolCallAssembler {
    calls: HashMap<String, PartialToolCall>,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    name: Option<String>,
    arguments: String,
}

impl ToolCallAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a tool call delta
    pub fn process_delta(
        &mut self,
        id: String,
        name: Option<String>,
        arguments_delta: Option<String>,
    ) {
        let call = self
            .calls
            .entry(id.clone())
            .or_insert_with(|| PartialToolCall {
                id: id.clone(),
                name: None,
                arguments: String::new(),
            });

        if let Some(n) = name {
            call.name = Some(n);
        }

        if let Some(delta) = arguments_delta {
            call.arguments.push_str(&delta);
        }
    }

    /// Get all completed tool calls
    pub fn into_tool_calls(self) -> Result<Vec<ToolCall>, serde_json::Error> {
        self.calls
            .into_iter()
            .map(|(_, partial)| {
                Ok(ToolCall {
                    id: partial.id,
                    name: partial.name.unwrap_or_default(),
                    arguments: serde_json::from_str(&partial.arguments)?,
                })
            })
            .collect()
    }
}

// ============================================================================
// Stream Chunk Types
// ============================================================================

/// Stream chunk for streaming responses
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// Text content delta
    Content(String),

    /// Final response with finish reason and usage
    Done {
        finish_reason: FinishReason,
        usage: TokenUsage,
        /// Full accumulated content
        full_content: String,
    },

    /// Thinking/reasoning content (for providers that support it)
    Thinking(String),

    /// Tool call in progress (for parallel tool calling support)
    /// Use the id to track which tool call this delta belongs to
    ToolCallDelta {
        /// Unique identifier for this tool call
        id: String,
        /// Tool name (sent once at the start)
        name: Option<String>,
        /// JSON arguments delta (accumulated to build full arguments)
        arguments_delta: Option<String>,
    },
}

// ============================================================================
// Chat Loop Types
// ============================================================================

/// Chat loop step - allows caller to handle tool execution
#[derive(Debug, Clone)]
pub enum LoopStep {
    /// LLM is thinking/reasoning (for providers that support extended thinking)
    Thinking(String),

    /// LLM is generating content (streamed text deltas)
    Content(String),

    /// LLM has requested tool calls (possibly multiple in parallel)
    ///
    /// Caller should:
    /// 1. Execute the requested tools
    /// 2. Call handle.submit_tool_results(results)
    /// 3. Continue calling handle.next() to receive more events
    ToolCallsRequested {
        /// All tool calls requested in this turn
        tool_calls: Vec<ToolCall>,
        /// Partial message content before tool calls (may be empty)
        content: String,
    },

    /// Tool results were received and processed by the LLM
    ///
    /// This is sent after the background task receives results via submit_tool_results()
    /// to confirm the loop is continuing. This is optional and mainly for UI feedback.
    ToolResultsReceived {
        /// Number of tool results received
        count: usize,
    },

    /// Final response (conversation complete)
    Done {
        /// Final content from the LLM
        content: String,
        /// Reason the conversation ended
        finish_reason: FinishReason,
        /// Total token usage for the entire loop
        total_usage: TokenUsage,
        /// All tool calls made during the loop
        all_tool_calls: Vec<ToolCall>,
    },
}

/// Handle for bidirectional chat loop communication
pub struct ChatLoopHandle {
    /// Stream of events from the LLM
    events: Pin<Box<dyn Stream<Item = Result<LoopStep, ProviderError>> + Send>>,

    /// Channel to send tool results back to the loop
    tool_result_tx: mpsc::UnboundedSender<ToolResultSubmission>,
}

impl ChatLoopHandle {
    /// Create a new ChatLoopHandle from channels
    #[allow(dead_code)]
    pub(crate) fn new(
        event_rx: mpsc::UnboundedReceiver<Result<LoopStep, ProviderError>>,
        tool_result_tx: mpsc::UnboundedSender<ToolResultSubmission>,
    ) -> Self {
        Self {
            events: Box::pin(UnboundedReceiverStream::new(event_rx)),
            tool_result_tx,
        }
    }

    /// Get the next event from the LLM
    pub async fn next(&mut self) -> Option<Result<LoopStep, ProviderError>> {
        use futures::StreamExt;
        self.events.next().await
    }

    /// Submit tool execution results to continue the loop
    ///
    /// This should be called after receiving `LoopStep::ToolCallsRequested`
    /// The background task will receive these results and continue the conversation
    pub fn submit_tool_results(&self, results: Vec<ToolResult>) -> Result<(), ProviderError> {
        self.tool_result_tx
            .send(ToolResultSubmission { results })
            .map_err(|_| ProviderError::ChatLoopClosed)?;
        Ok(())
    }

    /// Check if the loop is still active
    ///
    /// Returns false if the background task has finished or the channel is closed
    pub fn is_active(&self) -> bool {
        !self.tool_result_tx.is_closed()
    }

    /// Cancel the chat loop
    ///
    /// Drops the handle, which closes the channels and signals the background task to stop
    pub fn cancel(self) {
        // Dropping the handle will close the channels
        drop(self);
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Token usage statistics
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_tokens: u32,
}

impl TokenUsage {
    /// Calculate total tokens (input + output)
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Reason why generation finished
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}

// ============================================================================
// Error Types
// ============================================================================

/// Provider error types
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Prompt caching not supported")]
    CachingNotSupported,

    #[error("Tool calling not supported")]
    ToolCallingNotSupported,

    #[error("Chat loop closed")]
    ChatLoopClosed,

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
}
