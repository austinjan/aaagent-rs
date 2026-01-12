# Phase 6: Provider Refactoring - Remove History Management

**Date Started:** 2026-01-06  
**Date Completed:** 2026-01-06  
**Status:** ✅ COMPLETED  
**Priority:** 🔴 HIGH - Core infrastructure for tree-based history

---

## Objective

Remove history management from LLM providers, making them stateless. All history management will be handled by the Session/Agent layer using the tree-based system.

---

## Scope

### Remove from Providers
1. **Internal history field:** `Arc<RwLock<Vec<Message>>>`
2. **Trait methods:**
   - `fn get_history(&self) -> Vec<Message>`
   - `async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>>`
3. **History accumulation in chat_loop:** Stop storing messages internally
4. **Tool turn pruning:** Remove `prune_tool_turns()` methods

### Keep in Providers
- ✅ `chat()` - simple prompt → response
- ✅ `chat_loop()` - accepts Vec<Message>, returns stream
- ✅ `state()` - token usage tracking
- ✅ `config()` / `update_config()` - configuration
- ✅ Streaming support
- ✅ Tool calling support

---

## Changes Required

### 1. LLMProvider Trait (`src/llm/provider.rs`)

#### Remove Methods
```rust
// ❌ REMOVE
async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError>;
fn get_history(&self) -> Vec<Message>;
```

#### Keep Methods
```rust
// ✅ KEEP - These methods remain stateless
async fn chat(&self, prompt: &str) 
    -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;

async fn chat_loop(&self, history: Vec<Message>, tools: Option<Vec<Tool>>) 
    -> Result<ChatLoopHandle, ProviderError>;
```

### 2. OpenAIProvider (`src/llm/openai.rs`)

#### Remove Fields
```rust
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    config: Arc<RwLock<ProviderConfig>>,
    state: Arc<RwLock<ProviderState>>,
    
    // ❌ REMOVE
    // history: Arc<RwLock<Vec<Message>>>,
}
```

#### Remove Methods
```rust
// ❌ REMOVE
async fn compact(&self, history: Vec<Message>) -> Result<Vec<Message>, ProviderError> { ... }
fn get_history(&self) -> Vec<Message> { ... }
fn prune_tool_turns(messages: &mut Vec<ChatMessage>, max_turns: usize) { ... }
```

#### Update chat_loop Implementation
```rust
// OLD (with history tracking)
async fn chat_loop(&self, history: Vec<Message>, ...) -> Result<ChatLoopHandle> {
    let provider_history = self.history.clone();  // ❌ Remove
    let mut current_history = history.clone();    // ❌ Remove
    
    // Inside loop:
    current_history.push(msg);  // ❌ Remove
    Self::prune_tool_turns(&mut messages, max_turns);  // ❌ Remove
    
    // At end:
    *provider_history.write() = current_history;  // ❌ Remove
}

// NEW (stateless)
async fn chat_loop(&self, history: Vec<Message>, ...) -> Result<ChatLoopHandle> {
    // ✅ Just use input history directly
    let mut messages: Vec<ChatMessage> = history.iter()
        .map(Self::convert_message)
        .collect();
    
    // ✅ No history accumulation
    // ✅ No pruning - tree layer handles context
    
    // Just make API calls and stream results
}
```

### 3. AnthropicProvider (`src/llm/anthropic.rs`)

Same pattern as OpenAI:
- Remove `history` field
- Remove `get_history()`, `compact()`, `prune_tool_turns()`
- Make `chat_loop` stateless

### 4. ProviderConfig (`src/llm/provider.rs`)

#### Remove Field
```rust
pub struct ProviderConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    // ...
    
    // ❌ REMOVE - Replaced by Session checkpoint system
    // pub max_tool_turns: Option<usize>,
}
```

### 5. Update Tests

#### Remove Tests
- `test_get_history_initially_empty()`
- `test_prune_tool_turns_*()` tests

#### Update Tests
- Tests that check history accumulation
- Tests that assume history is tracked internally

---

## Migration Impact

### For Agent Layer
✅ **No changes needed** - Agent already uses Session for history

### For Direct Provider Users
⚠️ **Breaking changes:**

**Before:**
```rust
let provider = OpenAIProvider::create(...)?;
let mut handle = provider.chat_loop(vec![], tools).await?;

// Later: get accumulated history
let history = provider.get_history();
```

**After:**
```rust
let provider = OpenAIProvider::create(...)?;
let session = Session::new(...).await?;  // NEW: Use Session for history

// Get history from session, not provider
let history = session.get_context().await?;
let mut handle = provider.chat_loop(history, tools).await?;
```

### For Examples
Most examples already use Agent, which handles this internally. Only low-level examples that directly use providers need updates.

---

## Implementation Steps

### Step 1: Update LLMProvider Trait ✅
- [x] Remove `compact()` method
- [x] Remove `get_history()` method
- [x] Update trait documentation

### Step 2: Refactor OpenAIProvider ✅
- [x] Remove `history` field from struct
- [x] Remove from `new()` constructor
- [x] Remove `compact()` implementation
- [x] Remove `get_history()` implementation
- [x] Remove `prune_tool_turns()` helper
- [x] Update `chat_loop()` to be stateless
- [x] Remove `max_tool_turns` usage

### Step 3: Refactor AnthropicProvider ✅
- [x] Same changes as OpenAI
- [x] Ensure prompt caching still works (uses different mechanism)

### Step 4: Refactor GeminiProvider ✅
- [x] Remove `history` field from struct
- [x] Remove from `new()` constructor
- [x] Remove `compact()` stub implementation
- [x] Remove `get_history()` implementation
- [x] Remove `prune_message_tool_turns()` helper
- [x] Update `chat_loop()` to be stateless

### Step 5: Update ProviderConfig ✅
- [x] Remove `max_tool_turns` field
- [x] Update Default implementation
- [x] Update documentation

### Step 6: Update Tests ✅
- [x] Remove history-related tests (5 tests removed)
- [x] Update chat_loop tests
- [x] Ensure stateless behavior is tested
- [x] All 76 tests passing

### Step 7: Update Examples ✅
- [x] Check `simple_agent.rs` - uses Agent, no changes needed
- [x] Examples already use Agent/Session pattern
- [x] No direct provider usage in examples

### Step 8: Update Documentation ✅
- [x] Create PHASE6_COMPLETION_SUMMARY.md
- [x] Update phase plan with completion status
- [x] Document breaking changes

---

## Testing Strategy

### Unit Tests
- ✅ Provider creation still works
- ✅ chat() method works (stateless, always has been)
- ✅ chat_loop() works with provided history
- ✅ Token usage tracking still works
- ❌ History accumulation (removed feature)

### Integration Tests
- ✅ Agent + Session + Provider integration
- ✅ Multi-turn conversations via Session
- ✅ Tool calling via Session/Agent
- ✅ Context extraction from Session tree

### Regression Tests
- ✅ All existing Agent-based examples work
- ✅ Tool execution still works
- ✅ Streaming still works

---

## Benefits

1. **Cleaner Separation:** Providers focus on API calls, Session handles history
2. **Tree Support:** History naturally supports branching via Session
3. **Checkpoint Support:** Compression happens in Session, not providers
4. **Simpler Code:** Remove ~200 lines of history management from each provider
5. **Better Testing:** Provider tests are simpler without state management

---

## Risks & Mitigations

### Risk 1: Breaking Direct Provider Usage
**Impact:** Medium - Users directly using providers will need to adopt Session  
**Mitigation:**
- Clear migration guide
- Examples showing new pattern
- Gradual deprecation (if versioning)

### Risk 2: Test Coverage Gaps
**Impact:** Low - Might miss edge cases during refactoring  
**Mitigation:**
- Run full test suite after each change
- Manual testing of examples
- Integration test with Agent layer

### Risk 3: Performance Regression
**Impact:** Low - Removing state should improve performance  
**Mitigation:**
- Benchmark before/after
- Profile memory usage
- Measure latency

---

## Success Criteria

- [x] All LLM providers are stateless (no history field)
- [x] LLMProvider trait has no history methods
- [x] All tests pass (76/76 tests passing, 5 history tests removed)
- [x] All examples work (via Agent/Session)
- [x] Documentation updated
- [x] No regression in functionality

## Completion Metrics

**Code Changes:**
- Files modified: 4 core files (provider.rs, openai.rs, anthropic.rs, gemini.rs)
- Lines removed: ~235 lines of history management code
- Tests removed: 5 history-specific tests
- Test results: 76/76 passing ✅

**Breaking Changes:**
- Direct provider usage now requires manual history management via Session
- `max_tool_turns` config removed (replaced by Session checkpoint system)
- `get_history()` and `compact()` methods removed from trait

**Benefits Achieved:**
1. ✅ Clean separation: Providers focus on API calls only
2. ✅ Simplified provider code (~235 lines removed)
3. ✅ Enabled tree-based history system
4. ✅ No internal state to manage or synchronize
5. ✅ Easier to test and reason about

---

## Timeline

**Estimated:** 4-6 hours  
**Actual:** ~3 hours (completed same day)

- Step 1-2: OpenAI refactoring (1 hour)
- Step 3-4: Anthropic + Gemini refactoring (1 hour)
- Step 5-6: Config and tests (0.5 hours)
- Step 7-8: Examples and docs (0.5 hours)

---

## Next Steps After Phase 6

After providers are stateless:
- **Phase 7:** Implement JSONL storage backend
- **Phase 8:** Advanced features (pruning, vacuum)
- **Phase 9:** CLI integration
- **Phase 10:** Documentation

---

## References

- Plan: `doc/plan/TREE_MESSAGE_MODEL_PLAN.md` (Phase 6)
- Completion Summary: `doc/plan/PHASE6_COMPLETION_SUMMARY.md`
- Providers: `src/llm/openai.rs`, `src/llm/anthropic.rs`, `src/llm/gemini.rs`
- Provider Trait: `src/llm/provider.rs`
- Agent Layer: `src/agent/mod.rs` (already uses Session)

---

**Started:** 2026-01-06  
**Completed:** 2026-01-06  
**Status:** ✅ COMPLETED
