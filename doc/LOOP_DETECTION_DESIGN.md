# Loop Detection Design

**Status**: ✅ **IMPLEMENTED** (Levels 1-2)  
**Location**: `src/llm/loop_detector.rs`  
**Integration**: `src/llm/helpers.rs` (ChatLoopConfig)  
**Examples**: `examples/loop_detection_demo.rs`

## Problem Statement

LLMs with tool calling can sometimes get stuck in repetitive patterns:

1. **Infinite loops**: Calling the same tool with same arguments repeatedly
2. **Oscillating loops**: Alternating between 2-3 tools without progress
3. **Similar retries**: Repeatedly calling tools with slightly different arguments that produce similar results

Examples:
- LLM calls `bash ls` → sees no files → calls `bash ls` again → repeat
- LLM calls tool A → gets error → calls tool B → calls tool A again → repeat
- LLM searches for "foo" → not found → searches for "fo0" → not found → searches for "f00" → repeat

## Detection Strategies (Ordered by Complexity)

### Level 1: Exact Duplicate Detection ⭐ (Simplest) - ✅ IMPLEMENTED

**Difficulty**: Easy  
**Implementation Time**: ~30 minutes  
**Dependencies**: None  
**Status**: ✅ Fully implemented and tested

Detect when the exact same tool call is made multiple times in succession.

```rust
struct LoopDetector {
    recent_calls: VecDeque<ToolCall>,
    window_size: usize,
    max_duplicates: usize,
}

fn detect_exact_duplicate(&self, call: &ToolCall) -> bool {
    let count = self.recent_calls.iter()
        .filter(|c| c.name == call.name && c.arguments == call.arguments)
        .count();
    count >= self.max_duplicates
}
```

**Pros**: 
- Simple, fast, no false positives
- Easy to understand and debug
- No external dependencies
- Clear error messages

**Cons**: 
- Doesn't catch variations or oscillating patterns
- LLM can bypass by slightly changing arguments

**Use Cases**:
- Catch obvious infinite loops
- Prevent accidental repeated calls
- Quick win for initial implementation

---

### Level 2: Pattern Detection ⭐⭐ (Medium) - ✅ IMPLEMENTED

**Difficulty**: Medium  
**Implementation Time**: ~2-3 hours  
**Dependencies**: None (pure algorithm)  
**Status**: ✅ Fully implemented and tested

Detect oscillating patterns (A → B → A → B) or cycles (A → B → C → A).

```rust
fn detect_pattern(&self) -> Option<Vec<usize>> {
    // Find repeating subsequences in recent_calls
    // E.g., [A, B, C, A, B, C] → pattern [A, B, C] repeats
    
    for pattern_len in 2..=self.window_size/2 {
        if self.has_repeating_pattern(pattern_len) {
            return Some(self.extract_pattern(pattern_len));
        }
    }
    None
}

fn has_repeating_pattern(&self, pattern_len: usize) -> bool {
    if self.recent_calls.len() < pattern_len * 2 {
        return false;
    }
    
    let pattern: Vec<_> = self.recent_calls.iter()
        .rev()
        .take(pattern_len)
        .collect();
    
    let prev_pattern: Vec<_> = self.recent_calls.iter()
        .rev()
        .skip(pattern_len)
        .take(pattern_len)
        .collect();
    
    pattern.iter().zip(prev_pattern.iter())
        .all(|(a, b)| a.name == b.name && a.arguments == b.arguments)
}
```

**Pros**: 
- Catches oscillating loops (A→B→A→B)
- No false positives if implemented correctly
- Works with tool name comparison only (simpler version)

**Cons**: 
- O(n²) complexity for longer patterns
- May miss patterns with slight variations
- Harder to tune (what pattern length to check?)

**Use Cases**:
- LLM alternating between two different tools
- Cyclic behavior across 2-3 tools
- Retries with different tools but same goal

---

### Level 3: Similarity-Based Detection ⭐⭐⭐ (Hard) - 📋 NOT IMPLEMENTED

**Difficulty**: Hard  
**Implementation Time**: ~1 day  
**Dependencies**: String similarity library (e.g., `strsim`)  
**Status**: 📋 Design only, not implemented yet

Detect when similar tool calls (same tool, similar arguments) are made repeatedly.

```rust
fn detect_similar_calls(&self, call: &ToolCall) -> bool {
    let similar_count = self.recent_calls.iter()
        .filter(|c| {
            c.name == call.name && 
            self.arguments_similarity(&c.arguments, &call.arguments) > 0.8
        })
        .count();
    similar_count >= self.max_similar
}

fn arguments_similarity(&self, args1: &Value, args2: &Value) -> f64 {
    match (args1, args2) {
        (Value::String(s1), Value::String(s2)) => {
            // Levenshtein distance normalized
            let distance = strsim::levenshtein(s1, s2);
            let max_len = s1.len().max(s2.len());
            if max_len == 0 { 1.0 } else { 1.0 - (distance as f64 / max_len as f64) }
        }
        (Value::Object(o1), Value::Object(o2)) => {
            // Deep comparison with field-level similarity
            let keys: HashSet<_> = o1.keys().chain(o2.keys()).collect();
            let total = keys.len() as f64;
            let matching = keys.iter()
                .filter(|k| {
                    let v1 = o1.get(*k);
                    let v2 = o2.get(*k);
                    match (v1, v2) {
                        (Some(a), Some(b)) => self.arguments_similarity(a, b) > 0.8,
                        _ => false,
                    }
                })
                .count() as f64;
            matching / total
        }
        (Value::Array(a1), Value::Array(a2)) => {
            // Array similarity
            if a1.len() != a2.len() { return 0.0; }
            let sum: f64 = a1.iter().zip(a2.iter())
                .map(|(v1, v2)| self.arguments_similarity(v1, v2))
                .sum();
            sum / a1.len() as f64
        }
        _ => if args1 == args2 { 1.0 } else { 0.0 },
    }
}
```

**Pros**: 
- Catches variations like "ls" vs "ls -la"
- More intelligent detection
- Can tune sensitivity

**Cons**: 
- Complex implementation
- Requires threshold tuning (what is "similar enough"?)
- Risk of false positives
- External dependency for string similarity

**Use Cases**:
- LLM trying slightly different command variations
- Small parameter changes that don't affect outcome
- Gradual argument modifications

---

### Level 4: Result-Based Detection ⭐⭐⭐⭐ (Very Hard) - 📋 NOT IMPLEMENTED

**Difficulty**: Very Hard  
**Implementation Time**: ~2-3 days  
**Dependencies**: String similarity + memory management  
**Status**: 📋 Design only, not implemented yet

Detect when tool calls produce the same or very similar results repeatedly.

```rust
struct CallRecord {
    call: ToolCall,
    result: ToolResult,
    timestamp: Instant,
}

struct LoopDetector {
    recent_records: VecDeque<CallRecord>,
    max_history: usize,
}

fn detect_repetitive_results(&self, result: &ToolResult) -> bool {
    let similar_results = self.recent_records.iter()
        .filter(|r| {
            r.result.is_error == result.is_error &&
            self.result_similarity(&r.result.content, &result.content) > 0.9
        })
        .count();
    similar_results >= self.max_similar_results
}

fn result_similarity(&self, content1: &str, content2: &str) -> f64 {
    // Handle different result types
    
    // Empty results
    if content1.is_empty() && content2.is_empty() {
        return 1.0;
    }
    
    // Exact match
    if content1 == content2 {
        return 1.0;
    }
    
    // Structural similarity (e.g., JSON comparison)
    if let (Ok(json1), Ok(json2)) = (
        serde_json::from_str::<Value>(content1),
        serde_json::from_str::<Value>(content2)
    ) {
        return self.json_similarity(&json1, &json2);
    }
    
    // Text similarity (fallback)
    let distance = strsim::levenshtein(content1, content2);
    let max_len = content1.len().max(content2.len());
    if max_len == 0 { 1.0 } else { 1.0 - (distance as f64 / max_len as f64) }
}
```

**Pros**: 
- Catches semantic loops (different calls, same outcome)
- Most intelligent detection
- Detects truly unproductive behavior

**Cons**: 
- Requires storing all results (memory intensive)
- Very complex to implement correctly
- Hard to tune similarity thresholds
- False positives when similar results are legitimate
- Need to handle different result formats (text, JSON, binary, etc.)

**Use Cases**:
- Different tool calls producing same error
- Multiple search strategies finding nothing
- Variations that all fail the same way

---

## Complexity Summary

| Strategy | Difficulty | Time | Dependencies | False Positives | False Negatives | Memory Usage |
|----------|-----------|------|--------------|-----------------|-----------------|--------------|
| Exact Duplicate | ⭐ Easy | 30min | None | Very Low | High | Low |
| Pattern Detection | ⭐⭐ Medium | 2-3h | None | Low | Medium | Low |
| Similarity-Based | ⭐⭐⭐ Hard | 1 day | strsim | Medium | Low | Medium |
| Result-Based | ⭐⭐⭐⭐ Very Hard | 2-3 days | strsim + complex logic | High | Very Low | High |

## Recommended Approach: Hybrid Strategy - ✅ IMPLEMENTED (Simplified)

**Implementation Status**: ✅ Implemented with Levels 1-2 only

The actual implementation uses a simplified hybrid strategy focusing on exact duplicate and pattern detection:

```rust
pub struct LoopDetector {
    config: LoopDetectorConfig,
    recent_calls: VecDeque<CallRecord>,
    detection_count: usize,
}

pub struct LoopDetectorConfig {
    // Window size for tracking recent calls
    pub window_size: usize,           // Default: 10
    
    // Exact duplicate detection
    pub max_duplicates: usize,        // Default: 2
    
    // Pattern detection
    pub min_pattern_length: usize,    // Default: 2
    pub max_pattern_length: usize,    // Default: 5
    pub min_pattern_repetitions: usize, // Default: 2
    
    // Graduated response
    pub actions: Vec<LoopAction>,     // Default: [Warn, Warn, Terminate]
}

#[derive(Debug, Clone)]
pub enum LoopType {
    ExactDuplicate { call: ToolCall, count: usize },
    Pattern { pattern: Vec<ToolCall>, repetitions: usize },
}

pub enum LoopAction {
    Continue,
    Warn,
    Terminate,
}

pub struct LoopDetection {
    pub detected: bool,
    pub loop_type: LoopType,
    pub confidence: f64,
    pub suggestion: String,
    pub action: LoopAction,
    pub detection_count: usize,
    pub warning_message: Option<String>,
}

impl LoopDetector {
    pub fn check(&mut self, call: &ToolCall) -> Option<LoopDetection> {
        // 1. Check exact duplicates (highest priority)
        if let Some(detection) = self.check_exact_duplicate(call) {
            return Some(detection);
        }
        
        // 2. Check patterns
        if let Some(detection) = self.check_pattern(call) {
            return Some(detection);
        }
        
        // Record this call
        self.record_call(call);
        
        None
    }
    
    fn check_exact_duplicate(&mut self, call: &ToolCall) -> Option<LoopDetection> {
        let count = self.recent_calls.iter()
            .filter(|r| r.call.name == call.name && r.call.arguments == call.arguments)
            .count();
        
        if count >= self.config.max_duplicates {
            self.detection_count += 1;
            let action = self.get_action();
            
            Some(LoopDetection {
                detected: true,
                loop_type: LoopType::ExactDuplicate { 
                    call: call.clone(), 
                    count: count + 1,
                },
                confidence: 1.0,
                suggestion: format!(
                    "The tool '{}' has been called {} times with identical arguments. \
                     Consider a different approach or ask the user for help.",
                    call.name, count + 1
                ),
                action,
                detection_count: self.detection_count,
                warning_message: self.create_warning_message(&format!("exact duplicate of '{}'", call.name)),
            })
        } else {
            None
        }
    }
    
    fn get_action(&self) -> LoopAction {
        let idx = (self.detection_count - 1).min(self.config.actions.len() - 1);
        self.config.actions[idx].clone()
    }
}
```

## Integration with ChatLoop - ✅ IMPLEMENTED

**Implementation Status**: ✅ Hybrid approach (graduated response + callback + warning injection)

The actual implementation combines multiple strategies:

```rust
pub struct ChatLoopConfig {
    // ... existing fields
    pub loop_detection: Option<LoopDetectorConfig>,
    pub on_loop_detected: Option<LoopDetectionCallback>,
}

// In chat_loop_with_tools
let mut loop_detector = config
    .loop_detection
    .as_ref()
    .map(|cfg| LoopDetector::with_config(cfg.clone()));

// Before executing tools, check each call
if let Some(ref mut detector) = loop_detector {
    for call in &tool_calls {
        if let Some(detection) = detector.check(call) {
            // Invoke callback if provided
            let should_continue = if let Some(ref callback) = config.on_loop_detected {
                callback(&detection)
            } else {
                // Default handling based on action
                match detection.action {
                    LoopAction::Continue => true,
                    LoopAction::Warn => {
                        // Inject warning as tool result
                        tool_results.push(ToolResult {
                            tool_call_id: call.id.clone(),
                            content: detection.warning_message.unwrap_or_default(),
                            is_error: false,
                        });
                        true
                    }
                    LoopAction::Terminate => false,
                }
            };
            
            if !should_continue {
                detector.clear();  // Reset state
                return Err(ProviderError::LoopDetected(detection.suggestion));
            }
        }
    }
}
```

**Key Features:**
- Graduated response: Warn → Warn → Terminate
- Optional callback for custom handling
- Warning messages injected as tool results
- Automatic state cleanup on termination

## Configuration Examples - ✅ IMPLEMENTED

### Conservative (Fewer false positives)

```rust
LoopDetectorConfig {
    window_size: 15,
    max_duplicates: 4,
    min_pattern_length: 3,
    max_pattern_length: 5,
    min_pattern_repetitions: 3,
    actions: vec![LoopAction::Warn, LoopAction::Warn, LoopAction::Warn, LoopAction::Terminate],
}
```

### Aggressive (Catch loops early) - Used in examples/loop_detection_demo.rs

```rust
LoopDetectorConfig {
    window_size: 10,
    max_duplicates: 1,  // Detect on 2nd duplicate
    min_pattern_length: 2,
    max_pattern_length: 4,
    min_pattern_repetitions: 2,
    actions: vec![LoopAction::Warn, LoopAction::Terminate],
}
```

### Balanced (Default implementation) - ✅ CURRENT DEFAULT

```rust
impl Default for LoopDetectorConfig {
    fn default() -> Self {
        Self {
            window_size: 10,
            max_duplicates: 2,
            min_pattern_length: 2,
            max_pattern_length: 5,
            min_pattern_repetitions: 2,
            actions: vec![LoopAction::Warn, LoopAction::Warn, LoopAction::Terminate],
        }
    }
}
```

## Alternative: LLM Self-Awareness - 📋 NOT IMPLEMENTED

**Status**: Design idea, not implemented in code

Instead of hard-coded detection, include loop awareness in tool descriptions:

```rust
// In tool description
"IMPORTANT: If you find yourself calling this tool repeatedly with similar arguments 
and getting similar results, you may be in a loop. Consider:
1. Trying a completely different approach
2. Asking the user for more information
3. Acknowledging the limitation and explaining what you've tried"
```

This relies on the LLM's ability to recognize its own patterns, which can work well with capable models.

**Note**: This could complement the implemented loop detector for better results.

## Metrics to Track - 📋 NOT IMPLEMENTED

**Status**: Design idea, not implemented yet

```rust
pub struct LoopDetectionStats {
    pub total_checks: usize,
    pub loops_detected: usize,
    pub false_positives: usize,  // User-reported
    pub by_type: HashMap<String, usize>,
    pub average_confidence: f64,
}
```

**Note**: Could be added in future for monitoring and tuning loop detection behavior.

## Testing Strategy - ✅ IMPLEMENTED

**Status**: ✅ 6 unit tests implemented in `src/llm/loop_detector.rs`

### Unit Tests - ✅ IMPLEMENTED

Actual tests implemented:

```rust
#[test]
fn test_exact_duplicate_detection() {
    // Tests that 3 identical calls trigger detection
}

#[test]
fn test_pattern_detection() {
    // Tests A→B→A→B pattern detection
}

#[test]
fn test_no_false_positives() {
    // Tests that different calls don't trigger
}

#[test]
fn test_graduated_response() {
    // Tests Warn → Warn → Terminate sequence
}

#[test]
fn test_clear_resets_state() {
    // Tests that clear() resets detection count
}

#[test]
fn test_window_size_limit() {
    // Tests that old calls are discarded
}
```

**Test Results**: All 6 tests passing (34/34 total project tests)

### Integration Tests - 📋 NOT IMPLEMENTED

Integration with actual chat_loop not tested yet. See `examples/loop_detection_demo.rs` for demonstration usage.

## Open Questions - ⚠️ IMPLEMENTATION DECISIONS MADE

1. **Should loop detection be enabled by default?** - ✅ DECIDED
   - **Decision**: Opt-in via `ChatLoopConfig.loop_detection = Some(config)`
   - Pro: Prevents runaway costs
   - Con: May interrupt legitimate use cases
   - **Rationale**: Let users decide based on their use case

2. **Should detection vary by tool?** - 📋 NOT IMPLEMENTED
   - Some tools (like search) might legitimately be called multiple times
   - Others (like file write) probably shouldn't repeat
   - **Current**: Same thresholds for all tools
   - **Future**: Could add per-tool configuration

3. **How to handle multi-tool loops?** - ✅ IMPLEMENTED
   - Tool A → Tool B → Tool A is detected via pattern detection
   - **Implementation**: Pattern detection handles cycles up to max_pattern_length (default: 5)

4. **Should we track global state across chat_loop calls?** - ✅ DECIDED
   - **Decision**: Each chat_loop is independent
   - LoopDetector is created per chat_loop call
   - User can call `detector.clear()` to reset mid-conversation
   - **Rationale**: Simpler, more predictable behavior

## Recommendations - ✅ PHASE 1 & 2 COMPLETED

**Phase 1: Start Simple** - ✅ COMPLETED
- ✅ Implement exact duplicate detection only
- ✅ Make it opt-in via config flag
- ✅ Default threshold: 2 duplicates (configurable)

**Phase 2: Add Intelligence** - ✅ COMPLETED
- ⚠️ Similarity-based detection - NOT implemented (deferred to Phase 3)
- ✅ Implement pattern detection for 2-3 tool cycles
- ✅ Make thresholds configurable
- ✅ Implement callback system for user control

**Phase 3: Semantic Understanding** - 📋 FUTURE WORK
- 📋 Track results and detect repetitive outcomes (Level 4)
- 📋 Add tool-specific detection rules
- 📋 Similarity-based detection (Level 3)

**Phase 4: ML-Based (Future)** - 📋 FUTURE WORK
- 📋 Train model to recognize unproductive patterns
- 📋 Predict loops before they happen
- 📋 Adaptive thresholds based on user feedback

---

## Summary

**What's Implemented**: ✅
- Exact duplicate detection (Level 1)
- Pattern detection (Level 2)
- Graduated response (Warn → Warn → Terminate)
- Callback support
- Warning message injection
- State cleanup on terminate
- Comprehensive unit tests

**What's Not Implemented**: 📋
- Similarity-based detection (Level 3)
- Result-based detection (Level 4)
- Per-tool configuration
- Metrics tracking
- Integration tests

**Files**:
- Implementation: `src/llm/loop_detector.rs` (400+ lines, 6 tests)
- Integration: `src/llm/helpers.rs` (ChatLoopConfig)
- Example: `examples/loop_detection_demo.rs`
