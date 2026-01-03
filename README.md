# aaagent

Unified LLM provider abstraction with streaming, tool calling, and agent support for Rust.

## Features

- **Multi-provider support**: OpenAI, Anthropic (Claude), Gemini
- **Streaming**: Real-time token streaming with SSE parsing
- **Tool calling**: Parallel tool execution with automatic result handling
- **Chat loop**: High-level abstraction for multi-turn conversations
- **Loop detection**: Prevent repetitive tool calling patterns
- **Tool registry**: Centralized tool management with lazy loading

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
aaagent = "0.1"
```

Or with specific providers:

```toml
[dependencies]
aaagent = { version = "0.1", default-features = false, features = ["openai", "anthropic"] }
```

## Quick Start

```rust
use aaagent::llm::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let provider = OpenAIProvider::create("gpt-4o".to_string(), api_key)?;

    let mut stream = provider.chat("Hello, world!").await?;
    
    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamChunk::Content(text) => print!("{}", text),
            StreamChunk::Done { .. } => break,
            _ => {}
        }
    }
    
    Ok(())
}
```

## Providers

| Provider | Status | Features |
|----------|--------|----------|
| OpenAI | Complete | Streaming, tools, chat loop |
| Anthropic | Complete | Streaming, tools, chat loop |
| Gemini | Partial | Basic structure only |

## Examples

See the [examples](./examples) directory:

- `openai_basic.rs` - Simple chat
- `simple_agent.rs` - Multi-turn tool calling
- `interactive_agent.rs` - Interactive chat with tools
- `loop_detection_demo.rs` - Loop detection in action

## License

MIT
