# LLM Provider Design

## Overview

The LLM provider system provides a unified interface for interacting with multiple LLM providers (OpenAI, Anthropic, Gemini, etc.) with support for streaming, tool calling, and conversation management.

## Core Trait

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn create(model: String, api_key: String) -> Result<Self, ProviderError>;
    fn state(&self) -> ProviderState;
    fn config(&self) -> ProviderConfig;
    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig));
    async fn chat(&self, prompt: &str) -> Result<Stream<StreamChunk>, ProviderError>;
    async fn chat_loop(&self, history: Vec<Message>, tools: Option<Vec<Tool>>) -> Result<ChatLoopHandle, ProviderError>;
    fn prompt_cache(&mut self, cache_prompt: String) -> Result<(), ProviderError>;
    async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;
    fn get_history(&self) -> Vec<Message>;
}
```

## History Management

### Problem

In long-running conversations with tool calling, the conversation history can grow very large:

1. **Tool results can be huge**: bash command outputs, file contents, API responses
2. **Multiple rounds accumulate**: each tool call adds assistant message + tool results
3. **Token limits**: exceeding context window or increasing costs

### Solutions

We provide two complementary strategies:

#### 1. Automatic Tool Call/Result Pruning

**Problem**: Tool results accumulate indefinitely in history, consuming tokens.

**Solution**: Only keep the most recent N tool call/result "turns" in history.

**Configuration**:
```rust
pub struct ProviderConfig {
    /// Maximum number of tool call/result turns to keep (default: 3)
    /// One turn = assistant message with tool_calls + corresponding tool results
    pub max_tool_turns: Option<usize>,
}
```

**How it works**:
- Identifies tool "turns" in the conversation
- A turn = `Assistant(with tool_calls)` + `Tool` + `Tool` + ... (all results)
- When limit is exceeded, removes oldest turns
- Automatically applied after each tool result submission

**Example**:
```rust
let mut provider = OpenAIProvider::create(model, api_key)?;

// Configure to keep last 5 tool turns (default is 3)
provider.update_config(|cfg| {
    cfg.max_tool_turns = Some(5);
});

// Or disable pruning
provider.update_config(|cfg| {
    cfg.max_tool_turns = None; // Keep all history
});
```

**Conversation before pruning (with max_tool_turns = 2)**:
```
User: "Check files"
Assistant: [calls bash tool]
Tool: [large file listing - 500 lines]

User: "Count them"  
Assistant: [calls bash tool]
Tool: [result: "42 files"]

User: "Check logs"
Assistant: [calls bash tool]
Tool: [huge log output - 2000 lines]  ← Turn 3 added

# After pruning: Turn 1 removed, only Turn 2 and 3 remain
```

#### 2. Manual History Compaction

**Problem**: The entire conversation (not just tool calls) becomes too long.

**Solution**: Use provider-specific APIs to compress history while preserving understanding.

**Interface**:
```rust
async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;
```

**Implementation by Provider**:

##### OpenAI (Responses API)
- Endpoint: `POST /v1/responses/compact`
- All user messages kept verbatim
- Assistant messages, tool calls, and results replaced with encrypted "compaction items"
- Preserves model's latent understanding
- Returns compacted history for next request

##### Anthropic (Planned)
- Uses context editing (automatic in Claude Sonnet 4.5+)
- Or LLM-based summarization

##### Others
- May use LLM-based summarization
- Or simple truncation strategies

**Example**:
```rust
let provider = OpenAIProvider::create("gpt-4o".to_string(), api_key)?;

// Build up a long conversation
let mut history = vec![/* many messages */];

// Manually compact when needed
let compacted = provider.compact(history).await?;
println!("Reduced from {} to {} messages", history.len(), compacted.len());

// Continue with compacted history
let handle = provider.chat_loop(compacted, tools).await?;
```

**When to use**:
- Long conversations approaching token limits
- Before expensive operations to reduce costs
- Periodically in long-running agent sessions

## Comparison

| Strategy | Automatic | Scope | Use Case |
|----------|-----------|-------|----------|
| Tool Pruning | Yes | Tool calls/results only | Prevent tool output accumulation |
| Compaction | Manual | Entire conversation | Long conversations, reduce costs |

## Retrieving Conversation History

After a `chat_loop` completes, you can retrieve the full conversation history including all tool calls and results:

```rust
let provider = OpenAIProvider::create(model, api_key)?;

// Start a chat loop
let mut handle = provider.chat_loop(history, tools).await?;

// Process the conversation...
while let Some(event) = handle.next().await {
    // Handle events...
}

// Get the complete history after conversation ends
let full_history = provider.get_history();
println!("Conversation had {} messages", full_history.len());

// You can save this history for later
save_to_database(&full_history)?;

// Or continue the conversation
let handle = provider.chat_loop(full_history, tools).await?;
```

**Important notes:**
- History is only tracked during `chat_loop`, not `chat()`
- History includes automatic pruning (respects `max_tool_turns` config)
- History is stored per provider instance
- Each `chat_loop` call updates the stored history

## Best Practices

### 1. Configure Tool Pruning Based on Task

```rust
// For simple tasks with few tool calls
provider.update_config(|cfg| cfg.max_tool_turns = Some(1));

// For complex multi-step tasks
provider.update_config(|cfg| cfg.max_tool_turns = Some(10));

// For debugging (keep everything)
provider.update_config(|cfg| cfg.max_tool_turns = None);
```

### 2. Compact Periodically in Long Sessions

```rust
let mut history = initial_messages;
let mut turn_count = 0;

loop {
    let handle = provider.chat_loop(history.clone(), tools).await?;
    
    // Process response...
    turn_count += 1;
    
    // Compact every 10 turns
    if turn_count % 10 == 0 {
        history = provider.compact(history).await?;
        println!("Compacted history at turn {}", turn_count);
    }
}
```

### 3. Monitor Token Usage

```rust
let state = provider.state();
println!("Tokens used: {} in, {} out", 
    state.input_tokens, 
    state.output_tokens
);

if state.input_tokens > 50000 {
    // Consider compacting
    history = provider.compact(history).await?;
}
```

## Implementation Details

### OpenAI Provider

**Tool Pruning**:
- Implemented in `prune_tool_turns()`
- Called after each tool result submission
- Preserves non-tool messages (user, assistant without tools)

**Compaction**:
- Uses Responses API `/v1/responses/compact` endpoint
- Converts between `Message` and `ResponsesInput` formats
- Handles opaque compacted items

### Future Providers

Anthropic, Gemini, and other providers will implement:
- `compact()` using provider-specific APIs
- Same `max_tool_turns` configuration for consistency

## References

- [OpenAI Conversation State](https://platform.openai.com/docs/guides/conversation-state)
- [OpenAI Responses API](https://platform.openai.com/docs/api-reference/responses)
- [Anthropic Context Engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
