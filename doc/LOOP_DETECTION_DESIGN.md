# Loop Detection Design

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

### Level 1: Exact Duplicate Detection ⭐ (Simplest)

**Difficulty**: Easy  
**Implementation Time**: ~30 minutes  
**Dependencies**: None

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

### Level 2: Pattern Detection ⭐⭐ (Medium)

**Difficulty**: Medium  
**Implementation Time**: ~2-3 hours  
**Dependencies**: None (pure algorithm)

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

### Level 3: Similarity-Based Detection ⭐⭐⭐ (Hard)

**Difficulty**: Hard  
**Implementation Time**: ~1 day  
**Dependencies**: String similarity library (e.g., `strsim`)

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

### Level 4: Result-Based Detection ⭐⭐⭐⭐ (Very Hard)

**Difficulty**: Very Hard  
**Implementation Time**: ~2-3 days  
**Dependencies**: String similarity + memory management

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

## Recommended Approach: Hybrid Strategy

Combine multiple strategies with different severity levels:

```rust
pub struct LoopDetector {
    // Configuration
    config: LoopDetectorConfig,
    
    // State
    recent_calls: VecDeque<CallRecord>,
    call_count_map: HashMap<CallSignature, usize>,
}

pub struct LoopDetectorConfig {
    // Exact duplicate detection
    pub max_exact_duplicates: usize,           // Default: 3
    pub exact_window_size: usize,              // Default: 10
    
    // Similar call detection
    pub max_similar_calls: usize,              // Default: 5
    pub similarity_threshold: f64,             // Default: 0.8
    pub similar_window_size: usize,            // Default: 15
    
    // Pattern detection
    pub enable_pattern_detection: bool,        // Default: true
    pub min_pattern_length: usize,             // Default: 2
    pub pattern_window_size: usize,            // Default: 20
    
    // Result-based detection
    pub enable_result_tracking: bool,          // Default: true
    pub max_similar_results: usize,            // Default: 4
    pub result_similarity_threshold: f64,      // Default: 0.9
}

#[derive(Debug, Clone)]
pub enum LoopType {
    ExactDuplicate { call: ToolCall, count: usize },
    SimilarCalls { calls: Vec<ToolCall>, similarity: f64 },
    Pattern { pattern: Vec<ToolCall>, repetitions: usize },
    RepetitiveResults { results: Vec<String>, count: usize },
}

pub struct LoopDetection {
    pub detected: bool,
    pub loop_type: LoopType,
    pub confidence: f64,
    pub suggestion: String,
}

impl LoopDetector {
    pub fn check(&mut self, call: &ToolCall, result: Option<&ToolResult>) -> Option<LoopDetection> {
        // 1. Check exact duplicates (highest priority)
        if let Some(detection) = self.check_exact_duplicate(call) {
            return Some(detection);
        }
        
        // 2. Check similar calls
        if let Some(detection) = self.check_similar_calls(call) {
            return Some(detection);
        }
        
        // 3. Check patterns
        if self.config.enable_pattern_detection {
            if let Some(detection) = self.check_pattern() {
                return Some(detection);
            }
        }
        
        // 4. Check repetitive results
        if self.config.enable_result_tracking {
            if let Some(result) = result {
                if let Some(detection) = self.check_repetitive_results(result) {
                    return Some(detection);
                }
            }
        }
        
        // Record this call
        self.record_call(call, result);
        
        None
    }
    
    fn check_exact_duplicate(&self, call: &ToolCall) -> Option<LoopDetection> {
        let count = self.recent_calls.iter()
            .take(self.config.exact_window_size)
            .filter(|r| r.call.name == call.name && r.call.arguments == call.arguments)
            .count();
        
        if count >= self.config.max_exact_duplicates {
            Some(LoopDetection {
                detected: true,
                loop_type: LoopType::ExactDuplicate { 
                    call: call.clone(), 
                    count 
                },
                confidence: 1.0,
                suggestion: format!(
                    "The tool '{}' has been called {} times with identical arguments. \
                     Consider a different approach or ask the user for help.",
                    call.name, count
                ),
            })
        } else {
            None
        }
    }
}
```

## Integration with ChatLoop

### Option 1: Automatic Termination

```rust
// In chat_loop_with_tools
let mut loop_detector = LoopDetector::new(config.loop_detection_config);

loop {
    // ... tool execution
    
    if let Some(detection) = loop_detector.check(&call, Some(&result)) {
        if detection.confidence > 0.8 {
            return Err(ProviderError::LoopDetected {
                loop_type: detection.loop_type,
                suggestion: detection.suggestion,
            });
        }
    }
}
```

### Option 2: Callback Notification

```rust
pub struct ChatLoopConfig {
    // ... existing fields
    pub on_loop_detected: Option<Box<dyn Fn(&LoopDetection) -> LoopAction + Send>>,
}

pub enum LoopAction {
    Continue,           // Ignore and continue
    TerminateWithError, // Stop with error
    InjectMessage(String), // Add a message to history (e.g., "You seem stuck...")
}
```

### Option 3: Inject Warning Message

```rust
if let Some(detection) = loop_detector.check(&call, Some(&result)) {
    if detection.confidence > 0.8 {
        // Inject a system message
        messages.push(Message {
            role: Role::System,
            content: format!(
                "WARNING: Loop detected. {} \
                 Please try a completely different approach or ask the user for clarification.",
                detection.suggestion
            ),
            ..Default::default()
        });
    }
}
```

## Configuration Examples

### Conservative (Fewer false positives)

```rust
LoopDetectorConfig {
    max_exact_duplicates: 5,
    max_similar_calls: 7,
    similarity_threshold: 0.9,
    enable_pattern_detection: false,
    enable_result_tracking: false,
}
```

### Aggressive (Catch loops early)

```rust
LoopDetectorConfig {
    max_exact_duplicates: 2,
    max_similar_calls: 3,
    similarity_threshold: 0.7,
    enable_pattern_detection: true,
    enable_result_tracking: true,
    max_similar_results: 3,
}
```

### Balanced (Recommended default)

```rust
LoopDetectorConfig {
    max_exact_duplicates: 3,
    max_similar_calls: 5,
    similarity_threshold: 0.8,
    enable_pattern_detection: true,
    min_pattern_length: 2,
    enable_result_tracking: true,
    max_similar_results: 4,
}
```

## Alternative: LLM Self-Awareness

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

## Metrics to Track

```rust
pub struct LoopDetectionStats {
    pub total_checks: usize,
    pub loops_detected: usize,
    pub false_positives: usize,  // User-reported
    pub by_type: HashMap<String, usize>,
    pub average_confidence: f64,
}
```

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_exact_duplicate_detection() {
    let mut detector = LoopDetector::new(config);
    
    let call = ToolCall { name: "bash", arguments: json!({"command": "ls"}) };
    
    assert_eq!(detector.check(&call, None), None);
    assert_eq!(detector.check(&call, None), None);
    let detection = detector.check(&call, None);
    assert!(detection.is_some());
    assert!(matches!(detection.unwrap().loop_type, LoopType::ExactDuplicate { .. }));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_loop_detection_in_chat_loop() {
    // Create a provider that always calls the same tool
    // Verify loop detection triggers
    // Check error message quality
}
```

## Open Questions

1. **Should loop detection be enabled by default?**
   - Pro: Prevents runaway costs
   - Con: May interrupt legitimate use cases

2. **Should detection vary by tool?**
   - Some tools (like search) might legitimately be called multiple times
   - Others (like file write) probably shouldn't repeat

3. **How to handle multi-tool loops?**
   - Tool A → Tool B → Tool A is harder to detect
   - Requires pattern matching across different tools

4. **Should we track global state across chat_loop calls?**
   - Currently each chat_loop is independent
   - Could track across entire conversation history

## Recommendations

**Phase 1: Start Simple**
- Implement exact duplicate detection only
- Make it opt-in via config flag
- Default threshold: 3 duplicates

**Phase 2: Add Intelligence**
- Add similarity-based detection
- Implement pattern detection for 2-3 tool cycles
- Make thresholds configurable

**Phase 3: Semantic Understanding**
- Track results and detect repetitive outcomes
- Add tool-specific detection rules
- Implement callback system for user control

**Phase 4: ML-Based (Future)**
- Train model to recognize unproductive patterns
- Predict loops before they happen
- Adaptive thresholds based on user feedback
