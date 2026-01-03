# Auto Compact Strategy Design

## Current State

### What exists:
1. **`compact()` trait method** - Must be called manually
   - OpenAI: Uses `/responses/compact` endpoint
   - Gemini: Not implemented (returns error)

2. **`prune_message_tool_turns()`** - Removes old tool call/result message pairs
   - Only affects tool-related messages
   - Not a true compaction/summarization

### What's missing:
- No automatic compaction trigger
- No token counting for threshold detection
- No integration in `chat_loop_with_tools`
- Gemini has no compact implementation

---

## Proposed Design

### 1. Configuration Options

Add to `ProviderConfig`:

```rust
pub struct ProviderConfig {
    // ... existing fields ...
    
    /// Maximum input tokens before triggering auto-compact
    /// None = disabled (default)
    pub auto_compact_threshold: Option<u32>,
    
    /// Target token count after compaction
    /// Default: 50% of threshold
    pub compact_target_tokens: Option<u32>,
    
    /// Compact strategy to use
    pub compact_strategy: CompactStrategy,
}

pub enum CompactStrategy {
    /// Use provider's native compact API (OpenAI Responses API)
    Native,
    /// Summarize old messages using LLM
    Summarize,
    /// Simple truncation - keep recent messages only
    Truncate,
    /// No auto-compact
    Disabled,
}
```

### 2. Token Counting

Add token estimation to `Message`:

```rust
impl Message {
    /// Estimate token count (rough: chars / 4)
    pub fn estimate_tokens(&self) -> u32 {
        let base = (self.content.len() / 4) as u32;
        let tool_tokens = self.tool_calls
            .as_ref()
            .map(|calls| calls.iter()
                .map(|c| (c.arguments.to_string().len() / 4) as u32)
                .sum::<u32>())
            .unwrap_or(0);
        base + tool_tokens + 4 // +4 for role/metadata overhead
    }
}

fn estimate_history_tokens(history: &[Message]) -> u32 {
    history.iter().map(|m| m.estimate_tokens()).sum()
}
```

### 3. Compact Strategies Implementation

#### Strategy A: Native (OpenAI only)
- Use existing `compact()` method
- Falls back to Summarize for non-supporting providers

#### Strategy B: Summarize
- Use LLM to summarize older conversation portions
- Keep recent N messages intact
- Replace older messages with a single System message containing summary

```rust
async fn summarize_compact(
    provider: &impl LLMProvider,
    history: Vec<Message>,
    keep_recent: usize,
) -> Result<Vec<Message>, ProviderError> {
    let (old, recent) = history.split_at(history.len().saturating_sub(keep_recent));
    
    if old.is_empty() {
        return Ok(history);
    }
    
    // Build summary prompt
    let summary_prompt = format!(
        "Summarize this conversation concisely, preserving key facts and decisions:\n\n{}",
        format_messages_for_summary(old)
    );
    
    // Get summary from LLM
    let summary = provider.chat(&summary_prompt).await?;
    
    // Build new history
    let mut new_history = vec![Message {
        role: Role::System,
        content: format!("[Previous conversation summary]\n{}", summary),
        tool_call_id: None,
        tool_calls: None,
    }];
    new_history.extend(recent.to_vec());
    
    Ok(new_history)
}
```

#### Strategy C: Truncate
- Simple: keep only the most recent N messages
- Optionally keep system messages

```rust
fn truncate_compact(history: Vec<Message>, keep_count: usize) -> Vec<Message> {
    let system_msgs: Vec<_> = history.iter()
        .filter(|m| m.role == Role::System)
        .cloned()
        .collect();
    
    let recent: Vec<_> = history.into_iter()
        .filter(|m| m.role != Role::System)
        .rev()
        .take(keep_count)
        .collect();
    
    system_msgs.into_iter()
        .chain(recent.into_iter().rev())
        .collect()
}
```

### 4. Integration in chat_loop_with_tools

```rust
pub async fn chat_loop_with_tools<P: LLMProvider>(
    provider: &P,
    mut messages: Vec<Message>,
    tools: Vec<Tool>,
    config: ChatLoopConfig,
) -> Result<ChatLoopResponse, ProviderError> {
    let provider_config = provider.config();
    
    // Check if auto-compact is needed before starting
    if let Some(threshold) = provider_config.auto_compact_threshold {
        let current_tokens = estimate_history_tokens(&messages);
        if current_tokens > threshold {
            crate::logger::log(format!(
                "[compact] triggered: {} tokens > {} threshold",
                current_tokens, threshold
            ));
            
            messages = auto_compact(
                provider,
                messages,
                provider_config.compact_strategy,
                provider_config.compact_target_tokens.unwrap_or(threshold / 2),
            ).await?;
            
            crate::logger::log(format!(
                "[compact] done: {} tokens remaining",
                estimate_history_tokens(&messages)
            ));
        }
    }
    
    // ... rest of existing implementation
}
```

### 5. Gemini Compact Implementation

Since Gemini doesn't have a native compact API, implement using Summarize strategy:

```rust
// In gemini.rs
async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError> {
    // Use summarize strategy
    summarize_compact(self, history, 10).await
}
```

---

## Implementation Plan

### Phase 1: Foundation
1. Add `CompactStrategy` enum to provider.rs
2. Add config fields: `auto_compact_threshold`, `compact_target_tokens`, `compact_strategy`
3. Add `estimate_tokens()` to Message
4. Add `estimate_history_tokens()` helper

### Phase 2: Strategies
1. Implement `truncate_compact()` - simplest, no API calls
2. Implement `summarize_compact()` - uses LLM
3. Update OpenAI `compact()` to handle strategy selection

### Phase 3: Integration
1. Add auto-compact check in `chat_loop_with_tools`
2. Add logging for compact events
3. Implement Gemini `compact()` using summarize strategy

### Phase 4: Testing
1. Add unit tests for token estimation
2. Add unit tests for each compact strategy
3. Add integration test with mock provider
4. Update `interactive_agent.rs` example to demonstrate auto-compact

---

## Usage Example

```rust
// Enable auto-compact at 8000 tokens, target 4000 after compact
provider.update_config(|cfg| {
    cfg.auto_compact_threshold = Some(8000);
    cfg.compact_target_tokens = Some(4000);
    cfg.compact_strategy = CompactStrategy::Summarize;
});

// chat_loop_with_tools will now auto-compact when needed
let response = chat_loop_with_tools(&provider, history, tools, config).await?;
```

---

## Open Questions

1. **Should compact happen before or after each loop iteration?**
   - Before: Prevents hitting token limits mid-conversation
   - After: Ensures response is captured before compacting

2. **How to handle tool call context after compaction?**
   - Tool results reference tool_call_id - summarization loses this
   - Option: Keep recent tool turns intact, only summarize older content

3. **Should we support provider-specific compact configs?**
   - OpenAI might prefer Native strategy
   - Gemini must use Summarize/Truncate

4. **Token estimation accuracy**
   - chars/4 is rough approximation
   - Could use tiktoken for OpenAI, but adds dependency
   - Provider-specific token counting methods?
