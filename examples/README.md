# LLM Provider Examples

This directory contains examples demonstrating the LLM provider features.

## Prerequisites

Set your OpenAI API key:
```bash
export OPENAI_API_KEY=sk-...
# or on Windows
set OPENAI_API_KEY=sk-...
```

## Examples

### 1. `interactive_agent.rs` - Interactive Conversation with Detailed Logging ⭐ **Recommended**

**Features demonstrated:**
- Interactive conversation loop (ask questions, get answers)
- Detailed tool call/result logging in real-time
- History tracking across multiple turns
- Commands: `history` to view conversation, `exit` to quit
- Uses gpt-5-nano for fast responses

**Run:**
```bash
cargo run --example interactive_agent --features openai
```

**What it does:**
- Starts an interactive chat session
- Shows detailed information for every tool call and result
- Displays token usage and turn statistics
- Type `history` to see full conversation
- Type `exit` or `quit` to end session

**Example interaction:**
```
╔════════════════════════════════════════════════════════════╗
║     Interactive AI Agent with Tool Calling (gpt-5-nano)   ║
╚════════════════════════════════════════════════════════════╝

┌─ Turn 1 ────────────────────────────────────────────
│ 👤 You: list files in current directory
│
│ 🤖 Assistant: Let me check the files for you.
│
│ 🔧 Tool Calls Requested (1 call):
│   1. Tool: bash
│      Command: ls -la
│
│ ⏳ Executing tools...
│
│ 🔍 Tool Call Details:
│   1. bash
│      Cmd: ls -la
│      Result: total 48
│              drwxr-xr-x  12 user  staff   384 Dec 30 10:00 .
│              drwxr-xr-x   5 user  staff   160 Dec 29 15:30 ..
│              ... (15 more lines)
│
│ 📊 Turn Summary:
│   - Rounds: 1
│   - Tool calls: 1
│   - Tokens: 123 in / 45 out
│   - History size: 3 messages
└──────────────────────────────────────────────────────

┌─ Turn 2 ────────────────────────────────────────────
│ 👤 You: history
└──────────────────────────────────────────────────────

╔════════════════════════════════════════════════════════════╗
║                  Conversation History                      ║
╚════════════════════════════════════════════════════════════╝
Total messages: 3

[Shows all messages with details...]
```

### 2. `simple_agent.rs` - Multi-turn Tool Calling with History Tracking

**Features demonstrated:**
- Multi-turn tool calling (agent makes multiple tool calls to complete tasks)
- Conversation history tracking with `get_history()`
- Automatic tool result pruning (configurable via `max_tool_turns`)
- Tool execution with timeout

**Run:**
```bash
cargo run --example simple_agent --features openai
```

**What it does:**
1. Asks the agent to list folders and analyze files (requires 2+ tool calls)
2. Agent automatically calls bash tool multiple times
3. Displays conversation history after completion
4. Shows how pruning keeps history manageable

**Expected output:**
```
🤖 Simple AI Agent - Multi-turn Tool Calling Test
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📝 Tasks:
Please help me with the following tasks:
1. List all children folders in the current directory
2. Check files in the current folder and give me a summary

🔧 Executing 1 tool(s):
   $ ls -d */
   
[Agent processes results and makes more calls...]

📊 Summary:
   Rounds: 3
   Tools called: 4
   Tokens: 1234 in, 567 out (total: 1801)

📜 Conversation History:
   Total messages: 10
   
   [1] 👤 User:
      Please help me with the following tasks:
      
   [2] 🤖 Assistant:
      Called 1 tool(s):
        - bash
        
   [3] 🔧 Tool Result:
      doc/ examples/ src/ target/ ... (256 chars total)
      
   ...
   
✅ Test Complete!
   - Multiple tool calls executed successfully
   - History tracked: 10 messages
   - Tool pruning configured: max 3 turns
```

### 2. `openai_basic.rs` - Simple Chat Completion

Basic streaming chat without tools.

```bash
cargo run --example openai_basic --features openai
```

### 3. `openai_tools.rs` - Manual Tool Calling (Low-level API)

Manual control over tool calling loop.

```bash
cargo run --example openai_tools --features openai
```

### 4. `openai_compact.rs` - History Compaction

Demonstrates using OpenAI's Responses API to compress conversation history.

```bash
cargo run --example openai_compact --features openai
```

## Key Concepts Tested

### History Management

All examples demonstrate different aspects of history management:

1. **Automatic Tool Pruning** (`simple_agent.rs`)
   - Keeps last N tool call/result turns
   - Configurable via `max_tool_turns`
   - Prevents unbounded growth

2. **History Retrieval** (`simple_agent.rs`)
   - Use `get_history()` to retrieve conversation after completion
   - Includes all messages after pruning
   - Can be saved or used to continue conversation

3. **Manual Compaction** (`openai_compact.rs`)
   - Use `compact()` for long conversations
   - Provider-specific compression
   - Reduces tokens while preserving understanding

### Tool Calling Patterns

1. **High-level Helper** (`simple_agent.rs`)
   - `chat_loop_with_tools()` - automatic tool execution
   - Best for most use cases

2. **Low-level Control** (`openai_tools.rs`)
   - Manual `chat_loop()` with event handling
   - Full control over tool execution
   - Educational purposes

## Troubleshooting

**Error: OPENAI_API_KEY not set**
```
Set the environment variable before running
```

**Error: Model not supported**
```
Only GPT-5+, o1, and gpt-4o models are supported
Change to: gpt-5-nano, gpt-4o, o1-preview, etc.
```

**Timeout errors**
```rust
// Increase bash tool timeout
let bash_tool = BashTool::new().with_timeout(60); // 60 seconds
```
