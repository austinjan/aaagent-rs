//! Loop detection for preventing repetitive tool calling patterns
//!
//! This module provides utilities to detect when an LLM gets stuck in loops:
//! - Exact duplicate detection: same tool + same arguments
//! - Pattern detection: oscillating patterns (A→B→A→B)

use super::ToolCall;
use serde_json::Value;
use std::collections::VecDeque;

/// Action to take when a loop is detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopAction {
    /// Continue execution, ignore the loop
    Continue,

    /// Inject a warning message to the LLM
    Warn,

    /// Terminate with an error
    Terminate,
}

/// Configuration for loop detection
#[derive(Debug, Clone)]
pub struct LoopDetectorConfig {
    /// Maximum number of exact duplicate calls allowed (default: 3)
    pub max_exact_duplicates: usize,

    /// Window size for checking exact duplicates (default: 10)
    pub exact_window_size: usize,

    /// Enable pattern detection (A→B→A→B) (default: true)
    pub enable_pattern_detection: bool,

    /// Minimum pattern length to detect (default: 2)
    pub min_pattern_length: usize,

    /// Maximum pattern length to check (default: 3)
    pub max_pattern_length: usize,

    /// Window size for pattern detection (default: 20)
    pub pattern_window_size: usize,

    /// Action to take on first loop detection (default: Warn)
    pub first_detection_action: LoopAction,

    /// Action to take on second loop detection (default: Warn)
    pub second_detection_action: LoopAction,

    /// Action to take on third+ loop detection (default: Terminate)
    pub third_detection_action: LoopAction,
}

impl Default for LoopDetectorConfig {
    fn default() -> Self {
        Self {
            max_exact_duplicates: 3,
            exact_window_size: 10,
            enable_pattern_detection: true,
            min_pattern_length: 2,
            max_pattern_length: 3,
            pattern_window_size: 20,
            first_detection_action: LoopAction::Warn,
            second_detection_action: LoopAction::Warn,
            third_detection_action: LoopAction::Terminate,
        }
    }
}

/// Type of loop detected
#[derive(Debug, Clone, PartialEq)]
pub enum LoopType {
    /// Exact duplicate: same tool called with same arguments repeatedly
    ExactDuplicate {
        /// The tool call being repeated
        call: ToolCall,
        /// Number of times it appeared
        count: usize,
    },

    /// Pattern: repeating sequence of tool calls
    Pattern {
        /// The repeating pattern
        pattern: Vec<ToolCall>,
        /// Number of times the pattern repeated
        repetitions: usize,
    },
}

/// Loop detection result
#[derive(Debug, Clone)]
pub struct LoopDetection {
    /// Whether a loop was detected
    pub detected: bool,

    /// Type of loop
    pub loop_type: LoopType,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,

    /// Suggested action or message
    pub suggestion: String,

    /// Recommended action to take
    pub action: LoopAction,

    /// Number of times loops have been detected in this session
    pub detection_count: usize,

    /// Warning message to inject (if action is Warn)
    pub warning_message: Option<String>,
}

/// Record of a tool call
#[derive(Debug, Clone)]
struct CallRecord {
    call: ToolCall,
}

/// Loop detector
pub struct LoopDetector {
    config: LoopDetectorConfig,
    recent_calls: VecDeque<CallRecord>,
    detection_count: usize,
}

impl LoopDetector {
    /// Create a new loop detector with default configuration
    pub fn new() -> Self {
        Self::with_config(LoopDetectorConfig::default())
    }

    /// Create a loop detector with custom configuration
    pub fn with_config(config: LoopDetectorConfig) -> Self {
        let capacity = config.exact_window_size.max(config.pattern_window_size);
        Self {
            config,
            recent_calls: VecDeque::with_capacity(capacity),
            detection_count: 0,
        }
    }

    /// Check for loops before executing a tool call
    ///
    /// Returns Some(LoopDetection) if a loop is detected, None otherwise
    pub fn check(&mut self, call: &ToolCall) -> Option<LoopDetection> {
        // 1. Check exact duplicates (highest priority)
        if let Some(mut detection) = self.check_exact_duplicate(call) {
            self.detection_count += 1;
            self.apply_action_policy(&mut detection);
            // Record the call even though we detected a loop
            self.record_call(call);
            return Some(detection);
        }

        // 2. Check patterns (A→B→A→B)
        if self.config.enable_pattern_detection {
            if let Some(mut detection) = self.check_pattern(call) {
                self.detection_count += 1;
                self.apply_action_policy(&mut detection);
                self.record_call(call);
                return Some(detection);
            }
        }

        // No loop detected, record this call
        self.record_call(call);
        None
    }

    /// Apply the action policy based on detection count
    fn apply_action_policy(&self, detection: &mut LoopDetection) {
        detection.detection_count = self.detection_count;

        // Determine action based on detection count
        let action = match self.detection_count {
            1 => self.config.first_detection_action,
            2 => self.config.second_detection_action,
            _ => self.config.third_detection_action,
        };

        detection.action = action;

        // Generate warning message if action is Warn
        if action == LoopAction::Warn {
            detection.warning_message = Some(self.generate_warning_message(detection));
        }
    }

    /// Generate a warning message for the LLM
    fn generate_warning_message(&self, detection: &LoopDetection) -> String {
        let ordinal = match self.detection_count {
            1 => "first",
            2 => "second",
            3 => "third",
            n => return format!("Loop detected {} times", n),
        };

        let loop_description = match &detection.loop_type {
            LoopType::ExactDuplicate { call, count } => {
                format!(
                    "You have called the tool '{}' with identical arguments {} times in a row",
                    call.name, count
                )
            }
            LoopType::Pattern {
                pattern,
                repetitions,
            } => {
                let tool_names: Vec<_> = pattern.iter().map(|c| c.name.as_str()).collect();
                format!(
                    "You are repeating a pattern of {} tool calls: [{}] (repeated {} times)",
                    pattern.len(),
                    tool_names.join(" → "),
                    repetitions
                )
            }
        };

        format!(
            "⚠️ LOOP DETECTION WARNING ({}): {}\n\n\
             This appears to be unproductive behavior. Please consider:\n\
             1. Trying a completely different approach\n\
             2. Asking the user for more information or clarification\n\
             3. Acknowledging the limitation and explaining what you've tried\n\
             4. Using a different tool or strategy\n\n\
             If you continue with the same pattern, the system may terminate the conversation.",
            ordinal, loop_description
        )
    }

    /// Check for exact duplicate tool calls
    fn check_exact_duplicate(&self, call: &ToolCall) -> Option<LoopDetection> {
        let count = self
            .recent_calls
            .iter()
            .rev()
            .take(self.config.exact_window_size)
            .filter(|r| Self::calls_equal(&r.call, call))
            .count();

        if count >= self.config.max_exact_duplicates {
            Some(LoopDetection {
                detected: true,
                loop_type: LoopType::ExactDuplicate {
                    call: call.clone(),
                    count: count + 1, // +1 for the current call
                },
                confidence: 1.0,
                suggestion: format!(
                    "The tool '{}' has been called {} times with identical arguments. \
                     This appears to be an infinite loop.",
                    call.name,
                    count + 1
                ),
                action: LoopAction::Warn, // Will be overridden by apply_action_policy
                detection_count: 0,       // Will be set by apply_action_policy
                warning_message: None,    // Will be set by apply_action_policy
            })
        } else {
            None
        }
    }

    /// Check for repeating patterns (A→B→A→B or A→B→C→A→B→C)
    fn check_pattern(&self, call: &ToolCall) -> Option<LoopDetection> {
        // Need at least min_pattern_length * 2 calls to detect a pattern
        let min_required = self.config.min_pattern_length * 2;
        if self.recent_calls.len() < min_required {
            return None;
        }

        // Try different pattern lengths
        for pattern_len in self.config.min_pattern_length..=self.config.max_pattern_length {
            if let Some(detection) = self.check_pattern_of_length(call, pattern_len) {
                return Some(detection);
            }
        }

        None
    }

    /// Check for a repeating pattern of specific length
    fn check_pattern_of_length(
        &self,
        call: &ToolCall,
        pattern_len: usize,
    ) -> Option<LoopDetection> {
        // Need pattern_len * 2 calls (not including current call)
        if self.recent_calls.len() < pattern_len * 2 {
            return None;
        }

        // Build the pattern from the most recent calls
        let recent: Vec<_> = self.recent_calls.iter().rev().take(pattern_len).collect();

        // Check if this pattern repeated before
        let prev_pattern: Vec<_> = self
            .recent_calls
            .iter()
            .rev()
            .skip(pattern_len)
            .take(pattern_len)
            .collect();

        // Compare patterns
        let matches = recent
            .iter()
            .zip(prev_pattern.iter())
            .all(|(a, b)| Self::calls_equal(&a.call, &b.call));

        if !matches {
            return None;
        }

        // Check if the current call would continue the pattern
        // The current call should match the first call in the pattern
        if !Self::calls_equal(call, &recent[recent.len() - 1].call) {
            return None;
        }

        // Pattern detected! Extract it
        let pattern: Vec<ToolCall> = recent.iter().rev().map(|r| r.call.clone()).collect();

        Some(LoopDetection {
            detected: true,
            loop_type: LoopType::Pattern {
                pattern: pattern.clone(),
                repetitions: 2, // We found at least 2 repetitions
            },
            confidence: 1.0,
            suggestion: format!(
                "Detected a repeating pattern of {} tool calls. \
                 The pattern has repeated at least 2 times.",
                pattern_len
            ),
            action: LoopAction::Warn, // Will be overridden by apply_action_policy
            detection_count: 0,       // Will be set by apply_action_policy
            warning_message: None,    // Will be set by apply_action_policy
        })
    }

    /// Record a tool call in history
    fn record_call(&mut self, call: &ToolCall) {
        let max_size = self
            .config
            .exact_window_size
            .max(self.config.pattern_window_size);

        // Remove oldest if at capacity
        if self.recent_calls.len() >= max_size {
            self.recent_calls.pop_front();
        }

        self.recent_calls
            .push_back(CallRecord { call: call.clone() });
    }

    /// Check if two tool calls are equal (same name and arguments)
    fn calls_equal(a: &ToolCall, b: &ToolCall) -> bool {
        a.name == b.name && Self::arguments_equal(&a.arguments, &b.arguments)
    }

    /// Deep equality check for JSON arguments
    fn arguments_equal(a: &Value, b: &Value) -> bool {
        // serde_json::Value implements Eq, so we can use ==
        a == b
    }

    /// Get the number of recent calls tracked
    pub fn tracked_count(&self) -> usize {
        self.recent_calls.len()
    }

    /// Clear all tracked calls and reset detection count
    ///
    /// This should be called:
    /// - After terminating due to loop detection
    /// - When starting a new conversation
    /// - When you want to reset the loop detection state
    pub fn clear(&mut self) {
        self.recent_calls.clear();
        self.detection_count = 0;
    }

    /// Get the current detection count
    pub fn detection_count(&self) -> usize {
        self.detection_count
    }

    /// Reset only the detection count (keeps call history)
    pub fn reset_detection_count(&mut self) {
        self.detection_count = 0;
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_call(name: &str, args: Value) -> ToolCall {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        ToolCall {
            id: format!("call_{}", COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn test_exact_duplicate_detection() {
        let mut detector = LoopDetector::new();
        let call = make_call("bash", json!({"command": "ls"}));

        // First call - no loop
        assert!(detector.check(&call).is_none());

        // Second call - no loop yet
        assert!(detector.check(&call).is_none());

        // Third call - no loop yet (max is 3)
        assert!(detector.check(&call).is_none());

        // Fourth call - loop detected!
        let detection = detector.check(&call);
        assert!(detection.is_some());

        let detection = detection.unwrap();
        assert!(detection.detected);
        assert_eq!(detection.confidence, 1.0);

        match detection.loop_type {
            LoopType::ExactDuplicate { count, .. } => {
                assert_eq!(count, 4);
            }
            _ => panic!("Expected ExactDuplicate"),
        }
    }

    #[test]
    fn test_different_calls_no_loop() {
        let mut detector = LoopDetector::new();

        let call1 = make_call("bash", json!({"command": "ls"}));
        let call2 = make_call("bash", json!({"command": "pwd"}));
        let call3 = make_call("bash", json!({"command": "echo hello"}));

        assert!(detector.check(&call1).is_none());
        assert!(detector.check(&call2).is_none());
        assert!(detector.check(&call3).is_none());
        assert!(detector.check(&call1).is_none());
    }

    #[test]
    fn test_pattern_detection_ab_ab() {
        let mut detector = LoopDetector::new();

        let call_a = make_call("tool_a", json!({"param": "value_a"}));
        let call_b = make_call("tool_b", json!({"param": "value_b"}));

        // A, B
        assert!(detector.check(&call_a).is_none());
        assert!(detector.check(&call_b).is_none());

        // A, B, A - no pattern yet
        assert!(detector.check(&call_a).is_none());

        // A, B, A, B - pattern detected!
        assert!(detector.check(&call_b).is_none()); // Pattern not complete until we try to add A again

        // A, B, A, B, A - this should trigger pattern detection
        let detection = detector.check(&call_a);
        assert!(detection.is_some());

        let detection = detection.unwrap();
        assert!(detection.detected);
        match detection.loop_type {
            LoopType::Pattern {
                pattern,
                repetitions,
            } => {
                assert_eq!(pattern.len(), 2);
                assert_eq!(repetitions, 2);
            }
            _ => panic!("Expected Pattern"),
        }
    }

    #[test]
    fn test_pattern_detection_abc_abc() {
        let mut detector = LoopDetector::new();

        let call_a = make_call("tool_a", json!({"x": 1}));
        let call_b = make_call("tool_b", json!({"x": 2}));
        let call_c = make_call("tool_c", json!({"x": 3}));

        // First pattern: A, B, C
        assert!(detector.check(&call_a).is_none());
        assert!(detector.check(&call_b).is_none());
        assert!(detector.check(&call_c).is_none());

        // Second pattern: A, B, C
        assert!(detector.check(&call_a).is_none());
        assert!(detector.check(&call_b).is_none());
        assert!(detector.check(&call_c).is_none());

        // Start third pattern: A - should detect loop
        let detection = detector.check(&call_a);
        assert!(detection.is_some());

        match detection.unwrap().loop_type {
            LoopType::Pattern { pattern, .. } => {
                assert_eq!(pattern.len(), 3);
            }
            _ => panic!("Expected Pattern"),
        }
    }

    #[test]
    fn test_clear() {
        let mut detector = LoopDetector::new();
        let call = make_call("bash", json!({"command": "ls"}));

        detector.check(&call);
        detector.check(&call);
        assert_eq!(detector.tracked_count(), 2);

        detector.clear();
        assert_eq!(detector.tracked_count(), 0);

        // After clear, should not detect loop
        detector.check(&call);
        detector.check(&call);
        detector.check(&call);
        assert!(detector.check(&call).is_some()); // 4th call triggers loop
    }

    #[test]
    fn test_custom_config() {
        let config = LoopDetectorConfig {
            max_exact_duplicates: 2, // More aggressive
            ..Default::default()
        };

        let mut detector = LoopDetector::with_config(config);
        let call = make_call("bash", json!({"command": "ls"}));

        assert!(detector.check(&call).is_none());
        assert!(detector.check(&call).is_none());
        // Third call should trigger (max is 2)
        assert!(detector.check(&call).is_some());
    }
}
