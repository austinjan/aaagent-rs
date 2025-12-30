# LLM Provider Design Plan

## Overview

This document outlines the design for a flexible LLM provider system that supports **OpenAI**, **Anthropic**, and **Gemini** APIs with a unified interface.

## Core Architecture

### 1. Provider Trait Interface

```rust
pub trait LLMProvider: Send + Sync {
    /// Create a new provider instance with model and API key
    fn create(model: String, api_key: String) -> Result<Self, ProviderError> 
    where 
        Self: Sized;
    
    /// Get current provider state (token usage, request count, etc.)
    /// Thread-safe: can be called while streaming is active
    fn state(&self) -> ProviderState;
    
    /// Get mutable access to configuration
    fn config_mut(&mut self) -> &mut ProviderConfig;
    
    /// Get immutable access to configuration
    fn config(&self) -> &ProviderConfig;
    
    /// Simple chat completion (single prompt -> response)
    /// Returns a stream of chunks for streaming responses
    /// Uses &self (not &mut) for concurrent access
    async fn chat(
        &self, 
        prompt: &str
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
}

/// Handle for bidirectional chat loop communication
pub struct ChatLoopHandle {
    /// Stream of events from the LLM
    events: Pin<Box<dyn Stream<Item = Result<LoopStep, ProviderError>> + Send>>,
    
    /// Channel to send tool results back to the loop
    tool_result_tx: mpsc::UnboundedSender<ToolResultSubmission>,
}

impl ChatLoopHandle {
    /// Get the next event from the LLM
    pub async fn next(&mut self) -> Option<Result<LoopStep, ProviderError>> {
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
```

### 2. Supporting Data Structures

#### ProviderState
Tracks usage statistics and state information:
```rust
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

// Internal implementation detail: providers should use Arc<RwLock<ProviderState>>
// to allow thread-safe concurrent access during streaming
```

#### ProviderConfig
Configuration options for generation:
```rust
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
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            top_p: None,
            top_k: None,
            enable_reasoning: false,
            system_prompt: None,
            stop_sequences: Vec::new(),
            extra_options: HashMap::new(),
        }
    }
}
```

#### Message Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    
    /// Tool call ID (for tool responses)
    pub tool_call_id: Option<String>,
    
    /// Tool calls made by assistant
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}
```

#### Tool Definition
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
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
struct ToolResultSubmission {
    results: Vec<ToolResult>,
}

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
    pub fn process_delta(&mut self, id: String, name: Option<String>, arguments_delta: Option<String>) {
        let call = self.calls.entry(id.clone()).or_insert_with(|| PartialToolCall {
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
```

#### Response Types
```rust
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

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct ChatLoopResponse {
    pub final_message: String,
    pub tool_calls_made: Vec<ToolCall>,
    pub total_usage: TokenUsage,
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}
```

#### Error Handling
```rust
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
}
```

## 3. Implementation Structure

### Directory Structure
```
km-tools/
├── src/
│   ├── lib.rs              # Existing library code
│   ├── main.rs             # Existing CLI
│   └── llm/                # New LLM module
│       ├── mod.rs          # Module exports
│       ├── provider.rs     # Trait definition and common types
│       ├── openai.rs       # OpenAI implementation
│       ├── anthropic.rs    # Anthropic implementation
│       ├── gemini.rs       # Gemini implementation
│       └── utils.rs        # Shared utilities
└── Cargo.toml
```

### Provider Implementations

#### OpenAI Provider
- **Crate**: `async-openai` (official Rust SDK)
- **Features**:
  - ✅ Tool calling support
  - ❌ No native prompt caching (but has context caching in some models)
  - ✅ Streaming support
  - Models: GPT-4, GPT-4 Turbo, GPT-3.5 Turbo, o1, o1-mini
  
#### Anthropic Provider
- **Crate**: `anthropic-sdk` or `claude-rs` (community crates)
- **Features**:
  - ✅ Tool calling support
  - ✅ Prompt caching support
  - ✅ Extended thinking mode (Claude 3.5 Sonnet)
  - ✅ Streaming support
  - Models: Claude 3.5 Sonnet, Claude 3 Opus, Claude 3 Haiku

#### Gemini Provider
- **Crate**: `google-generative-ai-rs` (community) or manual HTTP client
- **Features**:
  - ✅ Tool calling support (Function Calling)
  - ✅ Context caching
  - ✅ Streaming support
  - Models: Gemini 1.5 Pro, Gemini 1.5 Flash, Gemini 2.0

### 4. Chat Loop Mechanism

#### 4.1 Bidirectional Communication Pattern

The `chat_loop` returns a `ChatLoopHandle` that enables bidirectional communication between the caller and the LLM provider using channels.

**Architecture**:
```
Caller (Your Code)                    Provider (Background Task)
     |                                         |
     | chat_loop(history, tools)               |
     |--------------------------------------->|
     |                                         |
     |<-- ChatLoopHandle                       |
     |    - events stream (rx)                 |
     |    - tool_result_tx                     |
     |                                         |
     | handle.next() -----------------------> | Send LoopStep::Content
     |<--------------------------------------- |
     |                                         |
     | handle.next() -----------------------> | Send LoopStep::ToolCallsRequested
     |<--------------------------------------- |
     |                                         |
     | [Execute tools]                         |
     |                                         |
     | handle.submit_tool_results(results) --> | Receive via tool_result_rx
     |                                         | Continue LLM API call with results
     |                                         |
     | handle.next() -----------------------> | Send LoopStep::Done
     |<--------------------------------------- |
```

**Implementation sketch**:
```rust
impl OpenAIProvider {
    async fn chat_loop(
        &self,
        history: Vec<Message>,
        tools: Option<Vec<Tool>>,
    ) -> Result<ChatLoopHandle, ProviderError> {
        // Create channels for bidirectional communication
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (tool_result_tx, tool_result_rx) = mpsc::unbounded_channel();
        
        // Spawn background task to drive the conversation
        let provider_clone = self.clone(); // Cheap Arc clone
        tokio::spawn(async move {
            provider_clone.run_chat_loop(
                history,
                tools,
                event_tx,
                tool_result_rx,
            ).await
        });
        
        // Return handle to caller
        Ok(ChatLoopHandle {
            events: Box::pin(ReceiverStream::new(event_rx)),
            tool_result_tx,
        })
    }
    
    async fn run_chat_loop(
        &self,
        mut history: Vec<Message>,
        tools: Option<Vec<Tool>>,
        event_tx: mpsc::UnboundedSender<Result<LoopStep, ProviderError>>,
        mut tool_result_rx: mpsc::UnboundedReceiver<ToolResultSubmission>,
    ) {
        loop {
            // Send API request
            let response = self.send_request(&history, &tools).await;
            
            match response.finish_reason {
                FinishReason::ToolCalls => {
                    // Extract tool calls
                    let tool_calls = extract_tool_calls(&response);
                    
                    // Notify caller
                    event_tx.send(Ok(LoopStep::ToolCallsRequested {
                        tool_calls: tool_calls.clone(),
                        content: response.content.clone(),
                    }));
                    
                    // Wait for tool results from caller
                    if let Some(submission) = tool_result_rx.recv().await {
                        // Optional: Notify caller that results were received
                        event_tx.send(Ok(LoopStep::ToolResultsReceived {
                            count: submission.results.len(),
                        }));
                        
                        // Add tool results to history
                        for result in submission.results {
                            history.push(Message {
                                role: Role::Tool,
                                content: result.content,
                                tool_call_id: Some(result.tool_call_id),
                                tool_calls: None,
                            });
                        }
                        // Continue loop with updated history
                    } else {
                        // Channel closed, exit
                        break;
                    }
                }
                FinishReason::Stop => {
                    event_tx.send(Ok(LoopStep::Done { ... }));
                    break;
                }
                _ => { /* handle other cases */ }
            }
        }
    }
}
```

#### 4.2 Old Implementation (Deprecated)

The previous blocking implementation looked like this:

```rust
async fn chat_loop(
    &mut self,
    history: &mut Vec<Message>,
    tools: Option<&[Tool]>,
) -> Result<ChatLoopResponse, ProviderError> {
    let mut tool_calls_made = Vec::new();
    let mut total_usage = TokenUsage::default();
    
    loop {
        // Send request with current history
        let response = self.send_request(history, tools).await?;
        
        // Update usage statistics
        total_usage.input_tokens += response.usage.input_tokens;
        total_usage.output_tokens += response.usage.output_tokens;
        total_usage.cached_tokens += response.usage.cached_tokens;
        
        match response.finish_reason {
            FinishReason::ToolCalls => {
                // Extract tool calls from response
                let tool_calls = extract_tool_calls(&response)?;
                tool_calls_made.extend(tool_calls.clone());
                
                // Add assistant message with tool calls to history
                history.push(Message {
                    role: Role::Assistant,
                    content: response.content,
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                });
                
                // Execute tools and add results to history
                for tool_call in tool_calls {
                    // User must implement tool execution externally
                    // This is where we'd pause and return control to caller
                    // For now, we'll require tool results to be provided
                    // in a callback or through a different mechanism
                }
                
                // Continue loop to get next response
            }
            FinishReason::Stop => {
                // Add final assistant message to history
                history.push(Message {
                    role: Role::Assistant,
                    content: response.content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                });
                
                return Ok(ChatLoopResponse {
                    final_message: response.content,
                    tool_calls_made,
                    total_usage,
                });
            }
            FinishReason::Length => {
                return Err(ProviderError::ApiError(
                    "Response truncated due to length".to_string()
                ));
            }
            _ => {
                return Err(ProviderError::ApiError(
                    format!("Unexpected finish reason: {:?}", response.finish_reason)
                ));
            }
        }
    }
}
```

**Note**: For proper tool calling, we might want to use a callback-based approach or return intermediate results. This needs refinement based on actual usage patterns.

### 5. Prompt Caching Implementation

Different providers have different caching mechanisms:

- **Anthropic**: Explicit prompt caching with cache control headers
- **OpenAI**: Automatic context caching in some models
- **Gemini**: Context caching API

```rust
fn prompt_cache(&mut self, cache_prompt: String) -> Result<(), ProviderError> {
    match self {
        // For Anthropic: Store the cached prompt and use cache_control
        AnthropicProvider => {
            self.cached_system_prompt = Some(cache_prompt);
            Ok(())
        }
        
        // For OpenAI: No explicit caching, but we can store for context
        OpenAIProvider => {
            self.cached_system_prompt = Some(cache_prompt);
            Ok(())
        }
        
        // For Gemini: Use context caching API
        GeminiProvider => {
            // Call Gemini's caching API
            self.create_cached_content(cache_prompt).await?;
            Ok(())
        }
    }
}
```

## 6. Dependencies to Add

```toml
[dependencies]
# Existing dependencies
clap = { version = "4", features = ["derive"] }
serde_json = "1"
ignore = "0.4"
jwalk = "0.8"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"

# New dependencies for LLM support
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
async-trait = "0.1"
reqwest = { version = "0.11", features = ["json", "stream"] }
thiserror = "1.0"
futures = "0.3"
pin-project = "1.0"

# Provider-specific (feature-gated)
async-openai = { version = "0.23", optional = true }
anthropic-sdk = { version = "0.2", optional = true }  # or use reqwest directly

[features]
default = ["openai", "anthropic", "gemini"]
openai = ["async-openai"]
anthropic = ["anthropic-sdk"]
gemini = []  # Use reqwest directly for Gemini
```

**Note**: `tokio::sync::mpsc` is included in `tokio` with the "sync" feature (part of "full").

## 7. Usage Examples

### 7.1 Streaming Chat (Basic)

```rust
use km_tools::llm::{LLMProvider, AnthropicProvider, StreamChunk};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create provider
    let mut provider = AnthropicProvider::create(
        "claude-3-5-sonnet-20241022".to_string(),
        std::env::var("ANTHROPIC_API_KEY")?
    )?;
    
    // Configure
    provider.config_mut().temperature = 0.5;
    provider.config_mut().enable_reasoning = true;
    
    // Streaming chat - note &self, not &mut self
    let mut stream = provider.chat("Explain Rust ownership in detail").await?;
    
    let mut full_response = String::new();
    
    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamChunk::Content(text) => {
                print!("{}", text);
                full_response.push_str(&text);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            StreamChunk::Thinking(thought) => {
                eprintln!("[Thinking: {}]", thought);
            }
            StreamChunk::Done { finish_reason, usage, full_content } => {
                println!("\n\n[Done: {:?}]", finish_reason);
                println!("Tokens: {} in / {} out", usage.input_tokens, usage.output_tokens);
            }
            StreamChunk::ToolCallDelta { .. } => {
                // Handle tool calls if needed (see example 7.4)
            }
        }
    }
    
    // Check state (thread-safe, can be called from another thread)
    let state = provider.state();
    println!("Total tokens used: {} input, {} output", 
             state.input_tokens, state.output_tokens);
    
    Ok(())
}
```

### 7.2 Chat Loop with Tools (Bidirectional Communication)

```rust
use km_tools::llm::{LLMProvider, OpenAIProvider, Message, Role, Tool, LoopStep, ToolResult};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = OpenAIProvider::create(
        "gpt-4-turbo".to_string(),
        std::env::var("OPENAI_API_KEY")?
    )?;
    
    // Initial history
    let history = vec![
        Message {
            role: Role::User,
            content: "What's the weather in Tokyo and Paris?".to_string(),
            tool_call_id: None,
            tool_calls: None,
        }
    ];
    
    let tools = vec![
        Tool {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        }
    ];
    
    // Start chat loop - returns a handle for bidirectional communication
    let mut handle = provider.chat_loop(history, Some(tools)).await?;
    
    while let Some(step) = handle.next().await {
        match step? {
            LoopStep::Thinking(thought) => {
                eprintln!("[Thinking: {}]", thought);
            }
            LoopStep::Content(text) => {
                print!("{}", text);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            LoopStep::ToolCallsRequested { tool_calls, content } => {
                println!("\n[Requested {} tool(s)]", tool_calls.len());
                
                // Execute tools in parallel
                let mut results = Vec::new();
                for call in &tool_calls {
                    println!("  - {}({:?})", call.name, call.arguments);
                    
                    // Execute tool
                    match execute_tool(&call.name, &call.arguments).await {
                        Ok(output) => {
                            results.push(ToolResult {
                                tool_call_id: call.id.clone(),
                                content: output,
                                is_error: false,
                            });
                        }
                        Err(e) => {
                            results.push(ToolResult {
                                tool_call_id: call.id.clone(),
                                content: format!("Error: {}", e),
                                is_error: true,
                            });
                        }
                    }
                }
                
                // Submit results back to the loop - this is the key!
                handle.submit_tool_results(results)?;
                // Loop continues, next event will be Content or Done
            }
            LoopStep::ToolResultsReceived { count } => {
                // Optional: confirmation that results were received
                println!("[Processing {} tool result(s)...]", count);
            }
            LoopStep::Done { content, finish_reason, total_usage, all_tool_calls } => {
                println!("\n\n[Done: {:?}]", finish_reason);
                println!("Total tool calls: {}", all_tool_calls.len());
                println!("Tokens: {} in / {} out", total_usage.input_tokens, total_usage.output_tokens);
                break;
            }
        }
    }
    
    // Can also check if loop is still active
    if handle.is_active() {
        println!("Loop is still active");
    }
    
    Ok(())
}

// Example tool execution function
async fn execute_tool(name: &str, args: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    match name {
        "get_weather" => {
            let city = args["city"].as_str().unwrap_or("Unknown");
            Ok(format!("The weather in {} is sunny, 22°C", city))
        }
        _ => Err("Unknown tool".into())
    }
}
```

### 7.3 Collecting Full Response from Stream

For cases where you want the full response instead of processing chunks:

```rust
use km_tools::llm::{LLMProvider, GeminiProvider, StreamChunk};
use futures::StreamExt;

async fn get_full_response(
    provider: &impl LLMProvider,
    prompt: &str
) -> Result<String, Box<dyn std::error::Error>> {
    let mut stream = provider.chat(prompt).await?;
    let mut full_content = String::new();
    
    while let Some(chunk) = stream.next().await {
        if let StreamChunk::Content(text) = chunk? {
            full_content.push_str(&text);
        }
    }
    
    Ok(full_content)
}
```

### 7.4 Handling Parallel Tool Calls in Streaming

When using `chat()` with tools enabled, handle parallel tool calls:

```rust
use km_tools::llm::{LLMProvider, OpenAIProvider, StreamChunk, ToolCallAssembler};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenAIProvider::create(
        "gpt-4-turbo".to_string(),
        std::env::var("OPENAI_API_KEY")?
    )?;
    
    let mut stream = provider.chat(
        "What's the weather in London, Tokyo, and New York?"
    ).await?;
    
    // Track parallel tool calls
    let mut assembler = ToolCallAssembler::new();
    let mut content = String::new();
    
    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamChunk::Content(text) => {
                print!("{}", text);
                content.push_str(&text);
            }
            StreamChunk::ToolCallDelta { id, name, arguments_delta } => {
                // Accumulate tool call deltas by ID
                assembler.process_delta(id, name, arguments_delta);
            }
            StreamChunk::Done { finish_reason, usage, .. } => {
                // Assemble all tool calls
                let tool_calls = assembler.into_tool_calls()?;
                
                println!("\n[Received {} tool call(s)]", tool_calls.len());
                for call in &tool_calls {
                    println!("  - {} with ID {}", call.name, call.id);
                    println!("    Args: {:?}", call.arguments);
                }
                
                // Execute tools in parallel
                let results = execute_tools_parallel(tool_calls).await?;
                
                println!("Tool results: {:?}", results);
            }
            _ => {}
        }
    }
    
    Ok(())
}

async fn execute_tools_parallel(
    calls: Vec<ToolCall>
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use futures::future::join_all;
    
    let futures: Vec<_> = calls.into_iter()
        .map(|call| async move {
            // Execute each tool
            execute_tool(&call.name, &call.arguments).await
        })
        .collect();
    
    join_all(futures).await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
}
```

## 8. Design Considerations

### Pros
- ✅ Unified interface for all providers
- ✅ Type-safe with Rust's type system
- ✅ Async-first design for performance
- ✅ Easy to swap providers
- ✅ Built-in state tracking
- ✅ Feature flags for minimal dependencies
- ✅ Thread-safe state access during streaming
- ✅ Support for parallel tool calling
- ✅ Event-driven chat loops for flexible control flow

### Production-Grade Refinements

#### A. Parallel Tool Calling Support
Modern LLMs (GPT-4, Claude 3.5, Gemini 2.0) can request multiple tools simultaneously.

**Solution**: 
- `ToolCallDelta` tracks calls by unique `id`
- `ToolCallAssembler` helper accumulates deltas into complete calls
- Callers use `HashMap<String, PartialToolCall>` internally
- See example 7.4 for implementation

#### B. Thread-Safe State Access
Reading state during active streaming requires interior mutability.

**Solution**:
- Trait methods use `&self` instead of `&mut self`
- Implementations use `Arc<RwLock<ProviderState>>` internally
- `state()` returns cloned `ProviderState` (cheap to clone)
- Allows concurrent state monitoring from other threads

**Implementation pattern**:
```rust
pub struct OpenAIProvider {
    client: Client,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
}

impl LLMProvider for OpenAIProvider {
    fn state(&self) -> ProviderState {
        self.state.read().unwrap().clone()
    }
}
```

#### C. Bidirectional Chat Loop with Tool Results
Blocking chat loops prevent caller control over tool execution, and streams alone can't receive feedback.

**Problem**: How does the caller submit tool results back to the loop?

**Solution - ChatLoopHandle with Channels**:
- `chat_loop()` returns `ChatLoopHandle` (not just a Stream)
- Handle contains:
  - `events`: Stream for receiving `LoopStep` events
  - `tool_result_tx`: Channel sender for submitting tool results
- Background task uses `tool_result_rx` to receive results and continue
- When `LoopStep::ToolCallsRequested` arrives, caller:
  1. Executes tools
  2. Calls `handle.submit_tool_results(results)`
  3. Background task receives results via channel
  4. Continues conversation with LLM

**Architecture**:
```rust
// Caller side
let mut handle = provider.chat_loop(history, tools).await?;
while let Some(step) = handle.next().await {
    match step? {
        LoopStep::ToolCallsRequested { tool_calls, .. } => {
            let results = execute_tools(tool_calls).await?;
            handle.submit_tool_results(results)?; // ← Send via channel
        }
        LoopStep::Done { .. } => break,
        _ => {}
    }
}

// Provider side (background task)
loop {
    let response = api_call().await;
    if response.has_tool_calls() {
        event_tx.send(LoopStep::ToolCallsRequested { ... });
        let results = tool_result_rx.recv().await; // ← Wait for results
        history.extend(results);
        continue; // Next API call
    }
}
```

**Benefits**:
- ✅ Bidirectional communication (events out, results in)
- ✅ Caller controls tool execution timing
- ✅ Non-blocking architecture
- ✅ Can cancel, timeout, or monitor progress
- ✅ Natural fit for event-driven agents
- ✅ Tool execution happens in caller's context with full error handling

### Challenges
- ⚠️ Different providers have different capabilities (need graceful degradation)
- ⚠️ Prompt caching APIs differ significantly between providers
- ⚠️ Streaming adds complexity to the API surface
- ⚠️ Tool execution coordination requires careful error handling

### Streaming Design Decisions

**Why streaming is the default for `chat()`:**
1. **Progressive output**: Users can display text as it arrives, better UX
2. **Early cancellation**: Can stop generation if needed
3. **Lower perceived latency**: First token arrives faster
4. **Flexibility**: Easy to collect into full response if needed (see example 7.3)
5. **Industry standard**: All major LLM providers support streaming

**Stream characteristics:**
- Uses Rust's `Stream` trait from `futures` crate
- Returns `Pin<Box<dyn Stream<...>>>` for trait object compatibility
- Each chunk is a `Result<StreamChunk, ProviderError>` for error handling
- Final chunk (`StreamChunk::Done`) contains usage statistics and finish reason

### Recommendations
1. **Start with basic implementation**: Get `create`, `chat`, `state`, and `config` working first
2. **Streaming implementation**: Build streaming support into `chat()` from the start
3. **Tool calling refinement**: Consider using an event-driven or callback approach for tool execution
4. **Testing**: Create integration tests with mock responses before testing against real APIs
5. **Error handling**: Implement retry logic with exponential backoff
6. **Helper utilities**: Provide utility functions to collect streams into full responses

## Next Steps

1. Implement the core trait and types in `provider.rs`
2. Implement OpenAI provider (easiest with official SDK)
3. Implement Anthropic provider
4. Implement Gemini provider
5. Add CLI commands to test the providers
6. Write comprehensive tests
7. Add documentation and examples
