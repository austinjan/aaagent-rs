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

## Tree-Based History Design

Unlike traditional chat applications that store conversations as linear message arrays, aaagent uses a **tree structure** for conversation history. This design provides several key advantages:

### Why Tree-Based History?

**1. Branching & Exploration**
- Explore alternative conversation paths from any point
- Try different approaches without losing previous work
- Example: Test multiple prompt variations from the same starting point

**2. Efficient Context Management**
- Checkpoint system: Summarize old conversations to reduce token usage
- Compression: Archive large tool results, retrieve on-demand
- Active path extraction: Only load relevant nodes for current conversation

**3. Non-Destructive Editing**
- Edit any message and continue from that point
- Original conversation path remains intact
- Multiple branches from the same parent message

**4. Replay & Debugging**
- Walk back through conversation history
- Inspect tool calls and results at any node
- Understand agent decision-making process

### Architecture

```
Agent (orchestrator)
├── Session (tree-based history)
│   ├── Root Node
│   ├── Active Leaf (current position)
│   ├── Checkpoints (context summaries)
│   ├── ContextOptimizationConfig
│   │   ├── CompressionConfig (tool result compression)
│   │   └── CheckpointConfig (auto-summarization)
│   └── TreeStore (JSONL persistence)
├── Provider (stateless LLM calls)
└── ToolRegistry (tool execution)
```

**Tree Structure Example:**
```
Root
 ├─ User: "Hello"
 │   ├─ Assistant: "Hi there!"
 │   │   └─ User: "How are you?"
 │   │       └─ Assistant: "I'm doing well!" (Branch A)
 │   │
 │   └─ Assistant: "Hello! How can I help?" (Branch B)
 │       └─ User: "Tell me about trees"
 │           └─ Assistant: "Trees are..." (Branch B active)
```

**Storage Format (JSONL):**
- `data/sessions/{session_id}.meta.json` - Session metadata
- `data/sessions/{session_id}.nodes.jsonl` - Append-only node log
- Each node contains: parent_id, role, content, tool_calls, timestamps
- Last entry wins for updates (flags, pruned_at)

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

## Configuration

### API Keys

Create `secrets.yaml` in the project root:

```yaml
api_keys:
  openai: "sk-..."
  anthropic: "sk-ant-..."
  gemini: "..."
```

### Session Configuration

Sessions can be configured with:
- System prompts
- Model selection (per provider)
- Temperature, max_tokens, top_p
- Context optimization settings
- Auto-checkpoint thresholds

See `config.yaml` for default configuration.

## License

MIT
