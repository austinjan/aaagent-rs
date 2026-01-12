# Examples

This directory contains examples demonstrating the aaagent library features.

## Prerequisites

Set your API key based on the provider you want to use:

```bash
# OpenAI
export OPENAI_API_KEY=sk-...

# Gemini
export GEMINI_API_KEY=...

# Anthropic
export ANTHROPIC_API_KEY=...
```

## Examples

### `interactive_agent_tree.rs` - Interactive Agent with Tree History

**Features demonstrated:**
- Tree-based conversation history (supports branching)
- Automatic checkpointing with context optimization
- Real-time event callbacks (tool calls, results, thinking, etc.)
- Tool execution via ToolRegistry
- Optional quick provider for simple tasks (checkpoint summaries)

**Run with OpenAI:**
```bash
cargo run --example interactive_agent_tree --features openai
```

**Run with Gemini:**
```bash
cargo run --example interactive_agent_tree --features gemini -- --provider=gemini
```

**With Quick Provider (uses cheaper model for checkpoints):**
```bash
QUICK_MODEL=gpt-4o-mini cargo run --example interactive_agent_tree --features openai
# or
QUICK_MODEL=gemini-2.0-flash-lite cargo run --example interactive_agent_tree --features gemini -- --provider=gemini
```

**Environment Variables:**
| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key | Required for OpenAI |
| `OPENAI_MODEL` | OpenAI model to use | `gpt-4o-mini` |
| `GEMINI_API_KEY` | Gemini API key | Required for Gemini |
| `GEMINI_MODEL` | Gemini model to use | `gemini-2.0-flash` |
| `QUICK_MODEL` | Model for simple tasks (checkpoints) | None (uses main model) |

**Commands during session:**
- Type your message to chat
- `branches` - View all conversation branches
- `checkpoints` - View checkpoint info
- `exit` or `quit` - End session

**Example interaction:**
```
╔════════════════════════════════════════════════════════════╗
║   Interactive AI Agent (Tree-based) - Gemini (gemini-2.0-flash)    ║
╚════════════════════════════════════════════════════════════╝

Features:
  - Tree-based conversation history (supports branching)
  - Automatic checkpointing (every 10 user turns)
  - Tool execution via ToolRegistry
  - Stateless provider (history in tree)
  - Quick provider enabled for checkpoint summaries

──── Turn 1 ────
👤 You: list files in current directory

>>> Event: ToolCallsRequested
    🔧 1 tool(s) requested:
       1. shell (id: call_abc123)
          {
            "command": "ls -la"
          }

>>> Event: ToolResult
    📦 ✓ shell (id: call_abc123)
       Result: total 48...

>>> Event: Done
    ✅ Completed
       Rounds: 1
       Tool calls: 1
       Tokens: 234 (input: 180, output: 54, cached: 0)

🤖 Assistant: Here are the files in the current directory...
```

### `loop_detection_demo.rs` - Loop Detection

**Features demonstrated:**
- Detection of repeated tool calls
- Pattern matching (A-B-A-B, A-B-C-A-B-C)
- Configurable thresholds and actions (warn, terminate)

**Run:**
```bash
cargo run --example loop_detection_demo --features openai
```

## Architecture

```
Agent (orchestrator)
├── Session (tree-based history)
│   ├── ContextOptimizationConfig
│   │   ├── CompressionConfig (tool result compression)
│   │   └── CheckpointConfig (context summarization)
│   └── TreeStore (storage backend)
├── Provider (stateless LLM calls)
│   └── quick_provider (optional, for simple tasks)
└── ToolRegistry (tool execution)
```

## Key Concepts

### Tree-based History
- Conversation stored as a tree, not a linear list
- Support for branching and replaying alternative paths
- Checkpoints summarize old conversation to reduce context

### Context Optimization
1. **Tool Result Compression** (3 layers)
   - Layer 1: Recent N turns - keep full
   - Layer 2: Medium age - truncate large results
   - Layer 3: Old - summarize, archive full content

2. **Checkpointing**
   - Auto-checkpoint after N user turns
   - Auto-checkpoint when context exceeds token limit
   - Summary replaces old conversation

### Quick Provider
- Optional cheaper/faster model for internal tasks
- Used for: checkpoint summaries, future compression tasks
- Falls back to main provider if not set

## Troubleshooting

**Error: API key not set**
```
Set the appropriate environment variable before running
```

**Error: Model not supported**
```
Check the supported models for your provider
OpenAI: gpt-4o, gpt-4o-mini, o1, etc.
Gemini: gemini-2.0-flash, gemini-1.5-pro, etc.
```
