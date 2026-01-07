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

### Development Mode (Recommended)

Start both frontend and backend with one command:

```bash
python develop.py start
```

This will:
- Start Vite dev server on http://localhost:5173 (hot reload)
- Start Rust backend on http://localhost:3000 (API + embedded UI)
- Manage both processes automatically

**Stop everything:**
```bash
python develop.py stop
```

**Restart backend only (after Rust code changes):**
```bash
python develop.py restart
```

### Manual Development Mode

If you prefer separate terminals:

**Terminal 1: Backend**
```bash
cargo run --features dev-server -- serve
# Runs on http://localhost:3000
```

**Terminal 2: Frontend**
```bash
cd web
npm run dev
# Runs on http://localhost:5173 with hot reload
# Proxies /api/* to backend on port 3000
```

### Production Build

Build a single binary that serves both UI and API:

```bash
# Step 1: Build frontend
cd web
npm run build

# Step 2: Build Rust binary (embeds frontend)
cd ..
cargo build --release

# Step 3: Run
./target/release/aaagent serve
# Serves on http://localhost:3000
```

## Testing

### Backend Health Check

```bash
curl http://localhost:3000/api/health
```

**Response:**
```json
{
  "status": "ok",
  "message": "aaagent-rs chat UI backend is running",
  "version": "0.1.0"
}
```

### Frontend

Open http://localhost:5173 (dev mode) or http://localhost:3000 (production)

## License

MIT
