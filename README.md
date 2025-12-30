---
description: Some tools for this KM, missing-readme reports folder without README.md, generating-map create km map.
---

# km-tools

CLI tools and LLM provider library for the km project.

## Build

```bash
cargo build --release
```

The binary will be at `target/release/km-tools`.

## Usage

### CLI Tools

```bash
km-tools --help
km-tools missing-readme --path .
km-tools generate-map --path . --format tree
```

### LLM Provider Library

See [examples/](examples/) for detailed usage examples.

## Quick Start - Interactive AI Agent ⭐

Try the interactive agent for hands-on experience with LLM + tools:

```bash
# Set your OpenAI API key
export OPENAI_API_KEY=sk-...

# Run interactive agent
cargo run --example interactive_agent --features openai
```

**Features:**
- Chat with AI agent in real-time
- Agent can execute bash commands via tools
- Detailed logging of every tool call and result
- Type `history` to view full conversation
- Type `exit` to quit

## LLM Provider Features

### History Management

The LLM provider automatically manages conversation history to prevent token overflow:

#### 1. Automatic Tool Call/Result Pruning

- **Tool Call/Result Pruning**: Only keeps the last 3 turns of tool calls and results by default
- **One turn** = one assistant message with tool_calls + all corresponding tool result messages
- **Configurable**: Set `max_tool_turns` in `ProviderConfig` to change the limit (or `None` for unlimited)

Example:
```rust
let mut provider = OpenAIProvider::create(model, api_key)?;
provider.update_config(|cfg| {
    cfg.max_tool_turns = Some(5); // Keep last 5 tool turns
    // or
    cfg.max_tool_turns = None; // Keep all tool history (unlimited)
});
```

This prevents large tool outputs (like bash command results) from consuming excessive tokens while maintaining recent context.

#### 2. Manual History Compaction

For long-running conversations, use the `compact()` method to compress conversation history:

```rust
let provider = OpenAIProvider::create(model, api_key)?;

// Long conversation
let history = vec![/* many messages */];

// Compact the history using provider-specific API
let compacted_history = provider.compact(history).await?;

// Use compacted history in next chat_loop
let mut handle = provider.chat_loop(compacted_history, tools).await?;
```

**Implementation by provider:**
- **OpenAI**: Uses the `/responses/compact` endpoint (Responses API)
- **Anthropic**: Uses context editing or summarization (coming soon)
- **Others**: May use LLM-based summarization or simple truncation

#### 3. Retrieving Conversation History

Get the full conversation history after a chat loop:

```rust
let provider = OpenAIProvider::create(model, api_key)?;
let mut handle = provider.chat_loop(history, tools).await?;

// Process conversation...
while let Some(event) = handle.next().await {
    // ...
}

// Get complete history (includes all messages after pruning)
let full_history = provider.get_history();
```

## Examples

### Interactive & Agent Examples

- **`interactive_agent.rs`** ⭐ - Interactive chat with detailed logging (Recommended)
- **`simple_agent.rs`** - Multi-turn tool calling test

### Core Features

- **`openai_basic.rs`** - Basic chat completion
- **`openai_tools.rs`** - Manual tool calling (low-level API)
- **`openai_compact.rs`** - History compaction

Run any example:
```bash
cargo run --example <name> --features openai
```

See [examples/README.md](examples/README.md) for detailed documentation.

## Documentation

- [doc/LLM_PROVIDER.md](doc/LLM_PROVIDER.md) - Complete LLM provider design and usage guide
- [examples/README.md](examples/README.md) - Example usage and tutorials

## Features

- **Unified LLM Interface**: Single API for OpenAI, Anthropic, Gemini (WIP)
- **Streaming Support**: Real-time token streaming
- **Tool Calling**: Function calling with automatic execution
- **History Management**: Automatic pruning and manual compaction
- **Thread-safe**: Concurrent access with `Arc<RwLock<>>`
