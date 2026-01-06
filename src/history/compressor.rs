use serde::{Deserialize, Serialize};

use super::node::{now, ArchivedToolResult};
use crate::llm::{Message, Role};

/// Configuration for context compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Tool results from last N turns are kept in full
    pub full_context_turns: usize,

    /// Tool results older than N turns are fully summarized
    pub summary_threshold_turns: usize,

    /// For medium-age results, truncate if larger than this (bytes)
    pub result_size_threshold: usize,

    /// Preview size for truncated results (bytes)
    pub preview_size: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            full_context_turns: 2,
            summary_threshold_turns: 10,
            result_size_threshold: 500,
            preview_size: 300,
        }
    }
}

/// Context compressor for optimizing tool call/result token usage
///
/// Implements a three-layer compression strategy:
/// - Layer 1: Recent (last N turns) - keep full
/// - Layer 2: Medium age (N to M turns) - truncate large results
/// - Layer 3: Old (> M turns) - summarize all
pub struct ContextCompressor {
    config: CompressionConfig,
}

impl ContextCompressor {
    /// Create a new compressor with the given configuration
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Compress tool calls and results in a message sequence
    ///
    /// Messages are analyzed to identify "turns" (User message boundaries),
    /// then tool calls/results are compressed based on their age.
    ///
    /// Archived results are stored in the provided HashMap for later retrieval
    /// via the recall_tool_result builtin tool.
    pub fn compress(
        &self,
        messages: Vec<Message>,
        archived_results: &mut std::collections::HashMap<String, ArchivedToolResult>,
    ) -> Vec<Message> {
        // Identify turn boundaries
        let turn_starts = Self::identify_tool_turns(&messages);

        let mut compressed = Vec::new();

        for (i, msg) in messages.into_iter().enumerate() {
            let turn_age = Self::calculate_turn_age(i, &turn_starts);

            match msg.role {
                Role::Tool => {
                    // Apply compression based on turn age
                    if turn_age < self.config.full_context_turns {
                        // Layer 1: Keep full
                        compressed.push(msg);
                    } else if turn_age < self.config.summary_threshold_turns {
                        // Layer 2: Truncate if large
                        if msg.content.len() > self.config.result_size_threshold {
                            let compressed_msg = self.truncate_tool_result(msg, archived_results);
                            compressed.push(compressed_msg);
                        } else {
                            compressed.push(msg);
                        }
                    } else {
                        // Layer 3: Full summary
                        let compressed_msg = self.summarize_tool_result(msg, archived_results);
                        compressed.push(compressed_msg);
                    }
                }
                Role::Assistant if msg.tool_calls.is_some() => {
                    // Compress tool calls in Layer 3
                    if turn_age >= self.config.summary_threshold_turns {
                        let compressed_msg = Self::summarize_tool_call(msg);
                        compressed.push(compressed_msg);
                    } else {
                        compressed.push(msg);
                    }
                }
                _ => {
                    // Keep other messages as-is
                    compressed.push(msg);
                }
            }
        }

        compressed
    }

    /// Truncate a tool result to preview size with recall hint
    fn truncate_tool_result(
        &self,
        msg: Message,
        archived_results: &mut std::collections::HashMap<String, ArchivedToolResult>,
    ) -> Message {
        let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
        let full_size = msg.content.len();

        // Archive the full result
        if let Some(ref tool_call_id_str) = msg.tool_call_id {
            archived_results.insert(
                tool_call_id_str.clone(),
                ArchivedToolResult {
                    tool_call_id: tool_call_id_str.clone(),
                    tool_name: "[tool result]".to_string(),
                    full_content: msg.content.clone(),
                    node_id: "".to_string(), // Will be filled by caller if needed
                    created_at: now(),
                    content_size: full_size,
                },
            );
        }

        // Create truncated content
        let preview: String = msg.content.chars().take(self.config.preview_size).collect();
        let truncated_content = format!(
            "{}...\n\n[Truncated. Original size: {} chars. Use recall_tool_result(tool_call_id='{}') to retrieve full content]",
            preview, full_size, tool_call_id
        );

        Message {
            role: msg.role,
            content: truncated_content,
            tool_call_id: msg.tool_call_id,
            tool_calls: msg.tool_calls,
        }
    }

    /// Summarize a tool result to minimal size
    fn summarize_tool_result(
        &self,
        msg: Message,
        archived_results: &mut std::collections::HashMap<String, ArchivedToolResult>,
    ) -> Message {
        let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
        let full_size = msg.content.len();

        // Archive the full result
        if let Some(ref tool_call_id_str) = msg.tool_call_id {
            archived_results.insert(
                tool_call_id_str.clone(),
                ArchivedToolResult {
                    tool_call_id: tool_call_id_str.clone(),
                    tool_name: "[tool result]".to_string(),
                    full_content: msg.content.clone(),
                    node_id: "".to_string(),
                    created_at: now(),
                    content_size: full_size,
                },
            );
        }

        let summary = format!(
            "[Tool result archived ({} chars). Use recall_tool_result(tool_call_id='{}') to retrieve]",
            full_size, tool_call_id
        );

        Message {
            role: msg.role,
            content: summary,
            tool_call_id: msg.tool_call_id,
            tool_calls: msg.tool_calls,
        }
    }

    /// Summarize a tool call to minimal size
    fn summarize_tool_call(msg: Message) -> Message {
        let summary = if let Some(tool_calls) = &msg.tool_calls {
            let calls: Vec<String> = tool_calls
                .iter()
                .map(|tc| format!("{}(...)", tc.name))
                .collect();
            format!("[Tool calls: {}]", calls.join(", "))
        } else {
            "[Tool call]".to_string()
        };

        Message {
            role: msg.role,
            content: summary,
            tool_call_id: msg.tool_call_id,
            tool_calls: msg.tool_calls,
        }
    }

    // ========================================================================
    // Turn Analysis
    // ========================================================================

    /// Identify tool turns in a message sequence
    ///
    /// A "turn" is defined as a sequence starting with a User message and ending
    /// just before the next User message. Returns indices of User messages that
    /// start each turn.
    fn identify_tool_turns(messages: &[Message]) -> Vec<usize> {
        messages
            .iter()
            .enumerate()
            .filter_map(|(i, msg)| {
                if msg.role == Role::User {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Calculate how many turns ago a message at `msg_index` was created
    ///
    /// Returns 0 for messages in the current turn (after last User message),
    /// 1 for messages in the previous turn, etc.
    fn calculate_turn_age(msg_index: usize, turn_starts: &[usize]) -> usize {
        // Find which turn this message belongs to
        let turn_index = turn_starts
            .iter()
            .enumerate()
            .rev() // Start from most recent turn
            .find(|(_, &start_idx)| start_idx <= msg_index)
            .map(|(turn_idx, _)| turn_idx);

        match turn_index {
            Some(idx) => {
                // Age = (total turns - 1) - turn_index
                // Last turn (most recent) has age 0
                turn_starts.len().saturating_sub(1).saturating_sub(idx)
            }
            None => {
                // Message before first User message (e.g., System prompt)
                // Consider it very old
                usize::MAX
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCall;

    #[test]
    fn test_identify_tool_turns() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Q1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "A1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::User,
                content: "Q2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let turns = ContextCompressor::identify_tool_turns(&messages);
        assert_eq!(turns, vec![0, 2]);
    }

    #[test]
    fn test_calculate_turn_age() {
        let turn_starts = vec![0, 3, 6]; // 3 turns

        // Turn 0 (oldest): age = 2
        assert_eq!(ContextCompressor::calculate_turn_age(0, &turn_starts), 2);
        assert_eq!(ContextCompressor::calculate_turn_age(1, &turn_starts), 2);
        assert_eq!(ContextCompressor::calculate_turn_age(2, &turn_starts), 2);

        // Turn 1 (middle): age = 1
        assert_eq!(ContextCompressor::calculate_turn_age(3, &turn_starts), 1);
        assert_eq!(ContextCompressor::calculate_turn_age(4, &turn_starts), 1);
        assert_eq!(ContextCompressor::calculate_turn_age(5, &turn_starts), 1);

        // Turn 2 (most recent): age = 0
        assert_eq!(ContextCompressor::calculate_turn_age(6, &turn_starts), 0);
        assert_eq!(ContextCompressor::calculate_turn_age(7, &turn_starts), 0);
    }

    #[test]
    fn test_calculate_turn_age_single_turn() {
        let turn_starts = vec![0];
        assert_eq!(ContextCompressor::calculate_turn_age(0, &turn_starts), 0);
        assert_eq!(ContextCompressor::calculate_turn_age(1, &turn_starts), 0);
    }

    #[test]
    fn test_compress_layer1_keeps_recent_full() {
        let mut archived = std::collections::HashMap::new();
        let config = CompressionConfig {
            full_context_turns: 2,
            ..Default::default()
        };
        let compressor = ContextCompressor::new(config);

        let messages = vec![
            Message {
                role: Role::User,
                content: "Q1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Calling tool".to_string(),
                tool_call_id: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "test_tool".to_string(),
                    arguments: serde_json::json!({}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: "A".repeat(1000),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Response".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let compressed = compressor.compress(messages.clone(), &mut archived);

        // Recent tool result should be kept in full
        assert_eq!(compressed[2].content.len(), 1000);
        assert!(!compressed[2].content.contains("Truncated"));
        assert!(archived.is_empty());
    }

    #[test]
    fn test_compress_layer2_truncates_large() {
        let mut archived = std::collections::HashMap::new();
        let config = CompressionConfig {
            full_context_turns: 1,
            summary_threshold_turns: 10,
            result_size_threshold: 100,
            preview_size: 50,
        };
        let compressor = ContextCompressor::new(config);

        let large_content = "A".repeat(200);
        let messages = vec![
            // Turn 0 (age = 1)
            Message {
                role: Role::User,
                content: "Q1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Calling tool".to_string(),
                tool_call_id: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "test_tool".to_string(),
                    arguments: serde_json::json!({}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: large_content.clone(),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Response".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            // Turn 1 (age = 0, recent)
            Message {
                role: Role::User,
                content: "Q2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let compressed = compressor.compress(messages.clone(), &mut archived);

        // Find the tool message
        let tool_msg = &compressed[2];
        assert_eq!(tool_msg.role, Role::Tool);

        // Should be truncated
        assert!(tool_msg.content.contains("Truncated"));
        assert!(tool_msg.content.contains("call_1"));
        assert!(tool_msg.content.contains("recall_tool_result"));

        // Should be archived
        assert!(archived.contains_key("call_1"));
        assert_eq!(archived.get("call_1").unwrap().full_content, large_content);
    }

    #[test]
    fn test_compress_layer3_summarizes_old() {
        let mut archived = std::collections::HashMap::new();
        let config = CompressionConfig {
            full_context_turns: 1,
            summary_threshold_turns: 2,
            ..Default::default()
        };
        let compressor = ContextCompressor::new(config);

        let messages = vec![
            // Turn 0 (age = 2, old)
            Message {
                role: Role::User,
                content: "Q1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Calling tool".to_string(),
                tool_call_id: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "test_tool".to_string(),
                    arguments: serde_json::json!({}),
                }]),
            },
            Message {
                role: Role::Tool,
                content: "Tool result".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Response 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            // Turn 1 (age = 1)
            Message {
                role: Role::User,
                content: "Q2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Response 2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            // Turn 2 (age = 0, recent)
            Message {
                role: Role::User,
                content: "Q3".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let compressed = compressor.compress(messages.clone(), &mut archived);

        // Tool call should be summarized
        let tool_call_msg = &compressed[1];
        assert!(tool_call_msg.content.contains("[Tool calls:"));

        // Tool result should be summarized
        let tool_result_msg = &compressed[2];
        assert!(tool_result_msg.content.contains("[Tool result archived"));
        assert!(tool_result_msg.content.contains("call_1"));

        // Should be archived
        assert!(archived.contains_key("call_1"));
        assert_eq!(archived.get("call_1").unwrap().full_content, "Tool result");
    }
}
