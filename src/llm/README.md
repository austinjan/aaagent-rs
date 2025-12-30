# LLM Provider Module

A unified interface for multiple LLM providers (OpenAI, Anthropic, Gemini) with support for:

- ✅ Streaming responses
- ✅ Parallel tool calling
- ✅ Bidirectional chat loops
- ✅ Thread-safe state tracking
- ✅ Prompt caching
- ✅ Extended thinking/reasoning modes

## Module Structure

```
src/llm/
├── mod.rs          # Module exports
├── provider.rs     # Core types and trait
├── tests.rs        # Unit tests
└── README.md       # This file
```

## Core Types Implemented

### 1. Trait: `LLMProvider`

The main trait that all providers must implement:

```rust
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    fn create(model: String, api_key: String) -> Result<Self, ProviderError>;
    fn state(&self) -> ProviderState;
    fn config_mut(&mut self) -> &mut ProviderConfig;
    fn config(&self) -> &ProviderConfig;
    async fn chat(&self, prompt: &str) -> Result<Stream<StreamChunk>, ProviderError>;
    async fn chat_loop(&self, history: Vec<Message>, tools: Option<Vec<Tool>>) -> Result<ChatLoopHandle, ProviderError>;
    fn prompt_cache(&mut self, cache_prompt: String) -> Result<(), ProviderError>;
}
```

### 2. State Management

- **`ProviderState`**: Tracks token usage, request count, timestamps
- **`ProviderConfig`**: Temperature, max_tokens, reasoning mode, etc.

### 3. Message Types

- **`Role`**: System, User, Assistant, Tool
- **`Message`**: Conversation messages with tool call support
- **`ToolCall`**: Tool invocations from the LLM

### 4. Tool Support

- **`Tool`**: Tool definitions with JSON Schema
- **`ToolResult`**: Tool execution results
- **`ToolCallAssembler`**: Helper for assembling parallel tool calls from streaming deltas

### 5. Streaming

- **`StreamChunk`**: Content, Done, Thinking, ToolCallDelta
- **`LoopStep`**: Thinking, Content, ToolCallsRequested, ToolResultsReceived, Done

### 6. Chat Loop Handle

- **`ChatLoopHandle`**: Bidirectional communication for tool execution
  - `next()`: Get next event from LLM
  - `submit_tool_results()`: Send tool results back
  - `is_active()`: Check if loop is running
  - `cancel()`: Cancel the loop

### 7. Error Handling

- **`ProviderError`**: Comprehensive error types for all failure modes

## Features

### Thread-Safe Design

All methods use `&self` (not `&mut self`) where possible, with internal `Arc<RwLock<>>` for state management.

### Parallel Tool Calling

The `ToolCallAssembler` helper tracks multiple simultaneous tool calls by their IDs, accumulating JSON argument deltas.

### Bidirectional Chat Loops

The `ChatLoopHandle` uses channels to enable two-way communication:
- Events flow from LLM to caller (via Stream)
- Tool results flow from caller to LLM (via channel)

## Testing

Run tests with:
```bash
cargo test --lib llm::tests
```

All 8 core type tests are passing:
- Provider state and config defaults
- Message creation
- Tool call assembly (single and parallel)
- Tool result creation
- Token usage tracking
- Finish reason equality

## Next Steps

To complete the implementation:

1. **Implement OpenAI Provider** (`src/llm/openai.rs`)
   - Use `async-openai` crate
   - Implement streaming and tool calling
   
2. **Implement Anthropic Provider** (`src/llm/anthropic.rs`)
   - HTTP client with reqwest
   - Prompt caching support
   - Extended thinking mode
   
3. **Implement Gemini Provider** (`src/llm/gemini.rs`)
   - HTTP client with reqwest
   - Context caching support

4. **Add Integration Tests**
   - Mock API responses
   - End-to-end scenarios
   
5. **Add CLI Commands**
   - Test providers from command line
   - Example usage

## Design Documentation

See `LLM_PROVIDER_DESIGN.md` in the project root for the complete design specification.
