# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**aaagent-rs** is a unified LLM provider abstraction library with streaming, tool calling, and agent orchestration. It supports OpenAI, Anthropic (Claude), and Gemini providers with a tree-based conversation history system.

## Architecture

### Core Components

```
Agent (orchestrator)
├── Session (tree-based history with branching)
│   ├── ContextOptimizationConfig
│   │   ├── CompressionConfig (tool result compression)
│   │   └── CheckpointConfig (context summarization)
│   └── TreeStore (storage backend: MemoryStore or custom)
├── Provider (stateless LLM calls - OpenAI, Anthropic, Gemini)
└── ToolRegistry (tool execution)
```

### Module Structure

- **`src/agent/`** - High-level Agent API orchestrating Session + Provider + Tools
- **`src/llm/`** - LLM provider implementations (OpenAI, Anthropic, Gemini) and tool calling
  - `provider.rs` - `LLMProvider` trait (stateless)
  - `openai.rs`, `anthropic.rs`, `gemini.rs` - Provider implementations
  - `registry.rs` - Tool registry and execution
  - `loop_detector.rs` - Detects repetitive tool calling patterns
- **`src/history/`** - Tree-based conversation history (Session)
  - `session.rs` - Main Session API with branching, checkpoints
  - `node.rs` - Tree node with parent/children relationships
  - `storage.rs` - Storage trait for persistence
  - `compressor.rs` - Tool result compression
  - `validator.rs` - Tree integrity validation
- **`src/web/`** - Embedded frontend asset serving (rust-embed)
- **`src/api/`** - REST API routes with axum
- **`src/explore_hierarchy.rs`** - Directory tree generation for LLM context
- **`src/logger.rs`** - Simple file-based logging

### Key Architectural Patterns

**Stateless Providers, Stateful Sessions:**
- Providers (`LLMProvider` trait) are stateless - they take linear message history and return responses
- Sessions manage the tree structure, branching, and context optimization
- Agent combines both: uses Session for tree operations, Provider for LLM calls

**Tree-Based History:**
- Conversations are stored as a tree, not a linear sequence
- Each message is a node with parent/child relationships
- Supports branching (exploring alternative responses) and replay
- Active leaf pointer tracks current conversation path

**Context Optimization:**
- **Compression**: Long tool results are replaced with summaries (hash reference to full content)
- **Checkpoints**: Conversation segments are summarized into checkpoint nodes
- Both reduce token usage while preserving retrievability via `recall_tool_result` tool

**Loop Detection:**
- Monitors tool call patterns to prevent infinite loops
- Three actions: Continue, Warn (inject warning into tool result), Terminate
- Configurable thresholds for identical calls and similar patterns

## Chat UI (Single Binary Architecture)

The project includes a React-based chat UI embedded in the Rust binary:

**Frontend:** Vite + React 18 + TypeScript + daisyUI + Tailwind CSS  
**Backend:** Rust + axum + rust-embed  
**Theme:** BlackBear TechHive (Yellow #E8C236, Black #000000)

### Build Process

**Development Mode:**
```bash
# Automated (recommended)
python develop.py start    # Start both frontend dev server (5173) + backend (3000)
python develop.py restart  # Rebuild/restart backend only
python develop.py stop     # Stop both

# Manual
cargo run --features dev-server -- serve  # Backend with CORS
cd web && npm run dev                      # Frontend with hot reload
```

**Production Mode:**
```bash
cargo build --release  # Auto-builds frontend via build.rs
./target/release/aaagent serve
```

The `build.rs` script automatically:
1. Detects release builds
2. Runs `npm install` if needed
3. Runs `npm run build` to compile frontend
4. Embeds `web/dist/` into binary via `rust-embed`

### Feature Flags

- `dev-server` - Enables CORS for development (backend serves on 3000, frontend proxies from 5173)
- `openai`, `anthropic`, `gemini` - LLM provider features (all enabled by default)

## Common Commands

### Testing
```bash
cargo test                    # Run all tests
cargo test --lib              # Library tests only
cargo test agent::tests       # Specific module
```

### CLI Tools
The binary includes CLI commands for directory analysis:

```bash
# Check for missing README files
cargo run -- missing-readme --path . --mk

# Generate hierarchical directory map
cargo run -- generate-map --path . --depth 3

# Start web server
cargo run -- serve --port 3000
```

### Development Workflow

**Backend changes:**
1. Edit Rust files
2. Run `python develop.py restart` (rebuilds + restarts backend, keeps frontend running)

**Frontend changes:**
1. Edit `web/src/` files
2. Vite auto-reloads (no restart needed)

**Adding dependencies:**
```bash
# Rust
cargo add <crate>

# Frontend
cd web && npm install <package>
```

## Important Implementation Details

### Provider Implementations Must Handle:
- Tool calls (parallel execution)
- Streaming tokens
- Token usage tracking
- SSE stream parsing (OpenAI, Gemini use SSE, Anthropic uses streaming JSON)

### Session Context Extraction:
When calling `session.get_context()`, it:
1. Traverses from active leaf up to root
2. Collects all ancestor nodes in path
3. Expands checkpoint summaries (if any)
4. Applies compression (replaces large tool results with summaries)
5. Returns linear message array for LLM provider

### Auto-Checkpointing Logic:
Checkpoints are created when:
- User turn count exceeds `every_n_turns` threshold (counts only User messages)
- Token count exceeds `every_n_tokens` threshold
- Active leaf doesn't already have a checkpoint (prevents duplicates)

### Tool Result Compression:
When a tool result exceeds `max_chars` threshold:
1. First 200 and last 100 chars are kept as preview
2. Full content stored in `archived_tool_results` map (keyed by tool_call_id)
3. LLM receives compressed version with "Use recall_tool_result to get full content"
4. LLM can call `recall_tool_result(tool_call_id)` to retrieve full content

## Web API Endpoints

All endpoints under `/api` prefix:

- `GET /api/health` - Health check (returns version, status)
- `POST /api/sessions/:id/chat` - Chat endpoint (placeholder)
- `GET /api/sessions/:id/stream/:stream_id` - SSE streaming (placeholder)
- `GET /api/sessions/:id/path` - Get active conversation path (placeholder)
- `GET /api/sessions/:id/checkpoints` - List checkpoints (placeholder)

Fallback handler serves embedded frontend from `web/dist/`.

## Project Conventions

### Error Handling
- Use `anyhow::Result` for application-level errors
- Use `thiserror` for library-level error types
- Async functions should return `Result<T>`

### Async Runtime
- Uses `tokio` with full features
- All LLM calls are async
- Session operations are async (to support future persistent storage)

### Logging
- Simple file-based logger in `src/logger.rs`
- Writes to `app.log` in current directory
- Use `aaagent::logger::log(message)` for logging

### Testing Strategy
- Unit tests in each module's `tests.rs` or inline `#[cfg(test)]`
- Integration tests use `MemoryStore` for in-memory history
- Mock providers for testing Agent without real API calls


