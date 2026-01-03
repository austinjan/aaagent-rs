# ToolRegistry Integration Design

> **Status**: PARTIALLY IMPLEMENTED  
> **Last Updated**: 2026-01-03

## Current Implementation Status

### ✅ Completed
- `ToolRegistry` struct with `register()`, `register_all_builtin()`, `execute()`
- `ToolProvider` trait for custom tools
- Integration in `chat_loop_with_tools()` - registry takes priority, fallback to `tool_executors`
- `LoopDetector` for preventing repetitive tool calling patterns

### ❌ Not Implemented
- `pick_tools` meta-tool for dynamic tool selection
- Brief vs full description switching
- Loop restart when tools change mid-conversation

---

## Original Problem Statement

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

---

## Solution: Option A - Restart Loop After pick_tools ⭐ (Recommended)

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

## Alternative: Option B - Add update_tools() to ChatLoopHandle

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

## Current Implementation (Partial)

The current `chat_loop_with_tools()` in `helpers.rs` supports:

```rust
// Registry takes priority, falls back to tool_executors
let result = if let Some(ref registry) = config.registry {
    if let Some(result) = registry.execute(call).await {
        result
    } else if let Some(executor) = config.tool_executors.get(&call.name) {
        // Tool not in registry, try executor
        // ...
    }
}
```

**What's missing:**
1. `pick_tools` meta-tool implementation
2. `registry.get_tools_for_llm()` returning brief descriptions initially
3. Loop restart logic when `pick_tools` is called
4. `registry.clear_picked()` method

---

## Implementation Plan (If Proceeding)

### Phase 1: ToolRegistry Enhancements
```rust
impl ToolRegistry {
    /// Track which tools have been "picked" for full descriptions
    picked_tools: HashSet<String>,
    
    /// Get tools for LLM - brief by default, full if picked
    pub fn get_tools_for_llm(&self) -> Vec<Tool> {
        self.tools.values().map(|provider| {
            let is_picked = self.picked_tools.contains(provider.name());
            Tool {
                name: provider.name().to_string(),
                description: if is_picked {
                    provider.full_description()
                } else {
                    provider.brief().to_string()
                },
                parameters: provider.parameters(),
            }
        }).collect()
    }
    
    /// Mark tools as picked (returns full descriptions next call)
    pub fn pick(&mut self, tool_names: &[String]) {
        for name in tool_names {
            self.picked_tools.insert(name.clone());
        }
    }
    
    /// Clear picked state (for conversation reset)
    pub fn clear_picked(&mut self) {
        self.picked_tools.clear();
    }
}
```

### Phase 2: pick_tools Meta-Tool
```rust
pub struct PickToolsTool {
    registry: Arc<RwLock<ToolRegistry>>,
}

impl ToolProvider for PickToolsTool {
    fn name(&self) -> &str { "pick_tools" }
    
    fn brief(&self) -> &str {
        "Select tools to get their full specifications"
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tool names to select"
                }
            },
            "required": ["tools"]
        })
    }
    
    fn execute(&self, call: &ToolCall) -> BoxFuture<Result<String, String>> {
        // Parse tool names from arguments
        // Call registry.pick(tool_names)
        // Return confirmation message
    }
}
```

### Phase 3: Nested Loop in chat_loop_with_tools
Implement the outer/inner loop pattern from Option A.

---

## Decision: Defer Implementation

**Rationale**: The current implementation is sufficient for most use cases:
- Direct tool registration works well
- Full descriptions are always available
- Loop detection prevents runaway tool calling

**When to revisit**:
- If token costs become prohibitive due to large tool descriptions
- If LLM confusion from many tool specs becomes an issue
- If dynamic tool selection is explicitly needed

---

## Example Usage (Current)

```rust
use km_tools::llm::*;
use km_tools::tools::*;

let provider = OpenAIProvider::from_env()?;

// Register all built-in tools
let registry = Arc::new(ToolRegistry::new().register_all_builtin());

let config = ChatLoopConfig::new()
    .with_registry(registry.clone())
    .on_content(|text| print!("{}", text))
    .on_tool_calls(|calls| {
        for call in calls {
            println!("📞 Calling: {} with {:?}", call.name, call.arguments);
        }
    });

let response = chat_loop_with_tools(
    &provider,
    vec![Message::user("List files in current directory")],
    registry.get_tools_for_llm(), // Pass tools explicitly
    config,
).await?;
```

---

## Open Questions (For Future Reference)

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
