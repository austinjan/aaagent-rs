# aaagent

Unified LLM provider abstraction with streaming, tool calling, and agent support for Rust.

## Features

- **Multi-provider support**: OpenAI, Anthropic (Claude), Gemini
- **Streaming**: Real-time token streaming with SSE parsing
- **Tool calling**: Parallel tool execution with automatic result handling
- **Tree-based history**: Branching conversations with checkpoints and replay
- **Context optimization**: Automatic compression of tool results and checkpointing
- **Loop detection**: Prevent repetitive tool calling patterns
- **Agent orchestration**: High-level Agent API combining Session + Provider + Tools

## Architecture

```
Agent (orchestrator)
├── Session (tree-based history)
│   ├── ContextOptimizationConfig
│   │   ├── CompressionConfig (tool result compression)
│   │   └── CheckpointConfig (context summarization)
│   └── TreeStore (storage backend)
├── Provider (stateless LLM calls)
└── ToolRegistry (tool execution)
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aaagent = "0.1"
```

Or with specific providers:

```toml
[dependencies]
aaagent = { version = "0.1", default-features = false, features = ["openai", "gemini"] }
```

## Quick Start

```rust
use aaagent::agent::Agent;
use aaagent::history::{MemoryStore, Session, SessionConfig};
use aaagent::llm::{OpenAIProvider, ToolRegistry};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let provider = OpenAIProvider::create("gpt-4o".to_string(), api_key)?;
    let store = Arc::new(MemoryStore::new());
    let config = SessionConfig::default();
    let session = Session::new(store, config).await?;
    let tools = ToolRegistry::new().register_all_builtin();

    // Create agent
    let mut agent = Agent::new(session, provider, tools);

    // Chat with real-time events
    let response = agent
        .chat_with_callback("List files in current directory", |event| {
            match event {
                AgentEvent::Content(text) => print!("{}", text),
                AgentEvent::ToolCallsRequested { tool_calls } => {
                    println!("Calling {} tools...", tool_calls.len());
                }
                AgentEvent::Done { total_usage, .. } => {
                    println!("\nTokens used: {}", total_usage.total());
                }
                _ => {}
            }
        })
        .await?;

    Ok(())
}
```

## Providers

| Provider | Status | Features |
|----------|--------|----------|
| OpenAI | Complete | Streaming, tools, chat loop |
| Anthropic | Complete | Streaming, tools, chat loop |
| Gemini | Complete | Streaming, tools, chat loop |

## Context Optimization

The library automatically optimizes context to reduce token usage:

### Tool Result Compression (3 layers)
- **Layer 1** (Recent): Last N turns kept in full
- **Layer 2** (Medium): Truncated with preview
- **Layer 3** (Old): Summarized, full content archived for recall

### Checkpointing
- Auto-checkpoint after N user turns
- Auto-checkpoint when context exceeds token limit
- Summaries replace old conversation history

## Examples

See the [examples](./examples) directory:

- `interactive_agent_tree.rs` - Interactive chat with tree history and tools
- `loop_detection_demo.rs` - Loop detection in action

Run with:
```bash
cargo run --example interactive_agent_tree --features gemini -- --provider=gemini
```

## License

MIT
