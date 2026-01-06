# Testing Tool Result Compression

This guide helps you test the three-layer tool result compression feature implemented in Phase 1-8.

## Quick Start

```bash
# Set your API key
export OPENAI_API_KEY="your-key-here"

# Run the interactive example
cargo run --example interactive_agent_tree --features openai

# Logs will be written to app.log
tail -f app.log  # In another terminal
```

## What to Test

### 1. **Layer 1: Recent Tool Results (Last 2 Turns)**

**Expected:** Tool results from the last 2 conversation turns should remain in full.

**Test Steps:**
1. Ask the agent to use a tool (e.g., "read the Cargo.toml file")
2. Check the logs - tool result should show as "Tool FULL"
3. Ask another question using a tool
4. The first tool result should still be FULL (within 2 turns)

**Log Pattern:**
```
[DEBUG] Tool FULL (XXX chars) - tool_call_id: Some("call_xxx")
```

### 2. **Layer 2: Medium-Age Tool Results (2-10 Turns)**

**Expected:** Large tool results (>500 chars) should be truncated to 300 char preview.

**Test Steps:**
1. Use a tool that returns large content (e.g., read a large file)
2. Have 2-3 more conversation turns
3. Check logs - old large result should show as "Tool TRUNCATED"

**Log Pattern:**
```
[INFO] Tool TRUNCATED (XXX chars) - tool_call_id: Some("call_xxx")
```

**In Context:**
```
[Truncated. Original size: 5000 chars. Use recall_tool_result(tool_call_id='call_xxx') to retrieve full content]
```

### 3. **Layer 3: Old Tool Results (>10 Turns)**

**Expected:** Very old tool results should be fully summarized.

**Test Steps:**
1. Use tools in early conversation
2. Have 10+ more conversation turns
3. Check logs - very old results should show as "Tool ARCHIVED"

**Log Pattern:**
```
[INFO] Tool ARCHIVED (XXX chars) - tool_call_id: Some("call_xxx")
```

**In Context:**
```
[Tool result archived (5000 chars). Use recall_tool_result(tool_call_id='call_xxx') to retrieve]
```

### 4. **Archived Results Storage**

**Expected:** Compressed tool results should be stored in archive for recall.

**Test Steps:**
1. Trigger compression (have old tool results)
2. Check logs for "Archived tool results: N stored"
3. Verify archived content sizes

**Log Pattern:**
```
Archived tool results: 3 stored
  - call_123 (5000 chars, created: 1234567890)
  - call_456 (3000 chars, created: 1234567891)
```

### 5. **Checkpoint Integration**

**Expected:** Checkpoints should work with compression (auto-created every 10 turns).

**Test Steps:**
1. Have 10+ conversation turns
2. Type `checkpoints` command
3. Check logs for checkpoint creation

**Log Pattern:**
```
Active checkpoints: 1
```

## Example Test Session

Here's a recommended test sequence:

```
Turn 1: "Read the Cargo.toml file"
  → Should use read_file tool
  → Result should be FULL (Layer 1)

Turn 2: "What's the project name?"
  → Previous tool result still FULL

Turn 3: "List files in the src directory"
  → New tool result FULL
  → Turn 1 result should still be FULL (within 2 turns)

Turn 4-5: Continue conversation without tools
  → Turn 1 result moves to Layer 2
  → If >500 chars, should be TRUNCATED

Turn 6-13: Continue conversation (mix of tool/non-tool)
  → Tool results age through layers
  → Watch for TRUNCATED → ARCHIVED transitions

Turn 14: Type "checkpoints"
  → Should see 1 checkpoint (created at turn 10)

Turn 15+: Continue testing
  → Very old tool results (>10 turns) should be ARCHIVED
```

## Log Analysis

### Configuration Check (at startup)

```
Session Configuration:
  Provider: OpenAI (gpt-4o-mini)
  Auto checkpoint every: Some(10) user turns
  Tool compression settings:
    - Full context turns: Some(2) (Layer 1)
    - Summary threshold turns: Some(10) (Layer 3)
    - Result size threshold: Some(500) chars (Layer 2)
    - Preview size: Some(300) chars (Layer 2)
```

### Per-Turn Analysis

```
Turn X: Processing user input
────────────────────────────────────────────────────────
Context Summary:
  Total messages: 15
  User: 5, Assistant: 5, Tool: 3, System: 2
  Total characters: 12500
  Tool results: 3 messages
    Total tool chars: 8500 (68.0% of context)
    Avg size: 2833, Min: 500, Max: 5000
  Archived tool results: 2 stored
────────────────────────────────────────────────────────
```

### Final Statistics

```
Final Session Statistics:
  Total turns: 15
  Tree nodes: 45
  Checkpoints created: 1
  Final context:
    - Total messages: 20
    - User: 6, Assistant: 6, Tool: 5
    - Total size: 15000 chars
  Compression statistics:
    - Truncated tool results: 2
    - Archived tool results: 1
    - Full tool results: 2
  Archived storage:
    - Total archived results: 3
    - Total archived content: 13000 chars
    - Average archived size: 4333 chars
```

## Success Criteria

✅ **Layer 1:** Recent tool results (last 2 turns) remain full
✅ **Layer 2:** Large results (>500 chars, 2-10 turns old) are truncated
✅ **Layer 3:** Old results (>10 turns) are fully archived
✅ **Archive:** Compressed results stored and retrievable
✅ **Checkpoints:** Created automatically every 10 turns
✅ **Tests:** All 81 tests passing

## Debugging Tips

1. **Check app.log for detailed traces**
   ```bash
   grep "Tool TRUNCATED\|Tool ARCHIVED" app.log
   ```

2. **Monitor compression in real-time**
   ```bash
   tail -f app.log | grep "Context Summary" -A 10
   ```

3. **Verify archive growth**
   ```bash
   grep "Archived tool results:" app.log
   ```

4. **Check for validation errors**
   ```bash
   grep "ERROR\|Orphaned\|Incomplete" app.log
   ```

## Known Edge Cases

- **Small tool results** (<500 chars) are never truncated, even in Layer 2
- **System messages** are never compressed
- **Tool calls** (Assistant messages with tool_calls) are only compressed in Layer 3
- **Empty tool results** are handled gracefully
- **Checkpoints** and compression work independently and don't interfere

## Architecture Verification

The refactoring (Phase 8) created three focused modules:

- **compressor.rs** (503 lines) - Compression strategies
- **validator.rs** (214 lines) - Message validation  
- **session.rs** (1168 lines) - Core tree management

All are tested and working together seamlessly.
