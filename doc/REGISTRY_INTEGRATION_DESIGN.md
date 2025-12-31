# ToolRegistry Integration Design

## Problem Statement

Current implementation has a fundamental architectural issue:

**Current Flow:**
```rust
chat_loop_with_tools(tools) 
  → provider.chat_loop(tools)  // tools are STATIC from this point
  → loop { execute tools }      // cannot update tools mid-loop
```

**Desired Flow:**
```
1. LLM sees: [brief tools] + pick_tools
2. LLM calls: pick_tools(["bash", "read_file"])
3. Tool result: "Selected bash, read_file. Full specs available next turn."
4. NEXT API CALL: [full bash spec] + [full read_file spec] + pick_tools
5. LLM can now call bash/read_file with proper schemas
```

## Root Cause

`ChatLoopHandle` from `provider.chat_loop()` **cannot update tools** mid-conversation:

```rust
// In OpenAI/Anthropic/etc providers:
pub async fn chat_loop(&self, messages: Vec<Message>, tools: Option<Vec<Tool>>) 
    -> Result<ChatLoopHandle, ProviderError> {
    
    // Tools are captured here and NEVER change
    let tools = tools;
    
    spawn(async move {
        loop {
            // Always uses the same 'tools' from closure
            let response = api_call(messages, tools.clone()).await;
            // ...
        }
    });
}
```

## Solution Options

### Option A: Restart Loop After pick_tools ⭐ (Recommended)

**Key Insight**: Instead of trying to update tools mid-loop, **restart the loop** with new tools.

```rust
pub async fn chat_loop_with_tools(
    provider: &P,
    mut messages: Vec<Message>,
    config: ChatLoopConfig,
) -> Result<ChatLoopResponse, ProviderError> {
    
    let mut registry = config.registry.expect("Registry required");
    let mut full_content = String::new();
    let mut all_tool_calls = Vec::new();
    let mut total_rounds = 0;
    
    loop {
        // Get current tools from registry
        let tools = registry.get_tools_for_llm();
        
        // Start a chat_loop with current tools
        let mut handle = provider.chat_loop(messages.clone(), Some(tools)).await?;
        
        while let Some(event) = handle.next().await {
            match event? {
                LoopStep::ToolCallsRequested { tool_calls, .. } => {
                    total_rounds += 1;
                    
                    // Execute tools
                    let mut results = Vec::new();
                    let mut picked_new_tools = false;
                    
                    for call in &tool_calls {
                        let result = registry.execute(call).await;
                        
                        // Check if this was a pick_tools call
                        if call.name == "pick_tools" && !result.is_error {
                            picked_new_tools = true;
                        }
                        
                        results.push(result);
                    }
                    
                    // Submit results
                    handle.submit_tool_results(results.clone())?;
                    
                    // Update messages with tool results
                    messages.push(Message::assistant_with_tools(tool_calls, ""));
                    messages.push(Message::tool_results(results));
                    
                    // If new tools were picked, RESTART the loop
                    if picked_new_tools {
                        drop(handle); // Close current loop
                        break; // Break inner loop, continue outer loop with new tools
                    }
                }
                LoopStep::Done { content, total_usage, .. } => {
                    registry.clear_picked();
                    return Ok(ChatLoopResponse {
                        content,
                        usage: total_usage,
                        all_tool_calls,
                        rounds: total_rounds,
                    });
                }
                _ => { /* handle other events */ }
            }
        }
    }
}
```

**Pros:**
- ✅ No changes to provider layer
- ✅ No changes to ChatLoopHandle
- ✅ Works with existing architecture
- ✅ Clear separation of concerns

**Cons:**
- ⚠️ Restarts underlying HTTP stream (minor overhead)
- ⚠️ More complex state management

---

### Option B: Add update_tools() to ChatLoopHandle

Add a channel to send tool updates:

```rust
pub struct ChatLoopHandle {
    events: Pin<Box<dyn Stream<...>>>,
    tool_result_tx: mpsc::UnboundedSender<ToolResultSubmission>,
    tool_update_tx: mpsc::UnboundedSender<ToolUpdateSubmission>, // NEW
}

impl ChatLoopHandle {
    pub fn update_tools(&self, tools: Vec<Tool>) -> Result<(), ProviderError> {
        self.tool_update_tx.send(ToolUpdateSubmission { tools })?;
        Ok(())
    }
}
```

**Pros:**
- ✅ More elegant API
- ✅ No loop restart overhead

**Cons:**
- ❌ Requires changes to ALL providers (OpenAI, Anthropic, etc.)
- ❌ Background task becomes more complex
- ❌ Breaking change to API

---

### Option C: Hybrid - registry.execute() in helpers, manual restart

Simplest implementation:

```rust
// In chat_loop_with_tools:

// Execute tools
for call in &tool_calls {
    let result = if let Some(ref mut registry) = config.registry {
        // Use registry.execute() which handles pick_tools
        registry.execute(call).await
    } else if let Some(executor) = config.tool_executors.get(&call.name) {
        // Fallback to legacy executors
        match executor(call.clone()).await {
            Ok(output) => ToolResult { ... },
            Err(error) => ToolResult { is_error: true, ... },
        }
    } else {
        ToolResult { is_error: true, content: "Tool not found" }
    };
    
    results.push(result);
}
```

**Issue**: Still doesn't solve the dynamic tools update problem.

---

## Recommended Implementation: Option A

### Implementation Plan

1. **Change `chat_loop_with_tools` signature**:
```rust
// OLD: accepts static tools
pub async fn chat_loop_with_tools(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,  // ← Static
    config: ChatLoopConfig,
)

// NEW: requires registry in config
pub async fn chat_loop_with_tools(
    provider: &P,
    messages: Vec<Message>,
    config: ChatLoopConfig,  // config.registry required
)
```

2. **Implement nested loop**:
   - Outer loop: Restart when tools change
   - Inner loop: Handle chat_loop events

3. **Track state across restarts**:
   - Accumulate messages
   - Accumulate tool calls
   - Count total rounds

4. **Detect pick_tools calls**:
   - When `call.name == "pick_tools"` and result is success
   - Set flag to restart outer loop

5. **Clear registry on completion**:
   - Call `registry.clear_picked()` when Done

### Migration Path

**Legacy support** (for users not using registry):

```rust
// Option 1: Separate function
pub async fn chat_loop_with_tools_legacy(
    provider: &P,
    messages: Vec<Message>,
    tools: Vec<Tool>,
    config: ChatLoopConfig,
) -> Result<...> {
    // Old implementation
}

// Option 2: Auto-create registry
pub async fn chat_loop_with_tools(
    provider: &P,
    messages: Vec<Message>,
    mut config: ChatLoopConfig,
) -> Result<...> {
    // If no registry, create one from tool_executors
    if config.registry.is_none() && !config.tool_executors.is_empty() {
        let mut registry = ToolRegistry::new();
        // Convert tool_executors to registry tools... (complex)
    }
}
```

**Better**: Make registry **optional** but recommended:

```rust
if let Some(mut registry) = config.registry {
    // New path: use registry with dynamic tools
    loop {
        let tools = registry.get_tools_for_llm();
        // ... nested loop logic
    }
} else {
    // Legacy path: static tools from tool_executors
    let tools = /* convert executors to tools */;
    let mut handle = provider.chat_loop(messages, Some(tools)).await?;
    // ... original logic
}
```

## Example Usage

```rust
use km_tools::llm::*;
use km_tools::tools::*;

let provider = OpenAIProvider::from_env()?;

let registry = ToolRegistry::new().register_all_builtin();

let config = ChatLoopConfig::new()
    .with_registry(registry)
    .on_content(|text| print!("{}", text));

let response = chat_loop_with_tools(
    &provider,
    vec![Message::user("List files in current directory")],
    config,
).await?;

// Expected flow:
// 1. LLM sees: [bash brief] + [read_file brief] + pick_tools
// 2. LLM calls: pick_tools(["bash"])
// 3. Result: "Selected bash. Full spec available next turn."
// 4. Loop restarts with: [bash FULL spec] + [read_file brief] + pick_tools
// 5. LLM calls: bash("ls")
// 6. Done!
```

## Open Questions

1. **Should we auto-restart or require user confirmation?**
   - Auto-restart is more convenient
   - But adds complexity and potential infinite loops

2. **How to handle message history?**
   - Keep all messages across restarts
   - Or summarize before restart?

3. **Should clear_picked() be automatic or manual?**
   - Auto: More convenient, per-conversation isolation
   - Manual: More control, can reuse tools across conversations

4. **Breaking change acceptable?**
   - If yes: Change signature now
   - If no: Add `chat_loop_with_registry()` alongside existing function
