use crate::llm::{Message, Role};
use anyhow::{anyhow, Result};

/// Message validator for conversation history
///
/// Validates structural constraints on message sequences,
/// particularly the Tool Sandwich pattern.
pub struct MessageValidator;

impl MessageValidator {
    /// Validate that tool results immediately follow their corresponding assistant messages.
    ///
    /// **Tool Sandwich Pattern**: `Assistant(tool_calls) → Tool(result)* → Assistant(response)`
    ///
    /// # Rules
    /// - Tool messages must immediately follow an Assistant message with tool_calls
    /// - Number of Tool results must match number of tool_calls
    /// - Tool results must appear before the next User or Assistant message
    ///
    /// # Errors
    /// - Returns error if tool results appear without preceding tool_calls (orphaned)
    /// - Returns error if tool_calls are not followed by expected number of results (incomplete)
    pub fn validate_tool_sandwich(messages: &[Message]) -> Result<()> {
        let mut expecting_tool_results = false;
        let mut tool_calls_count = 0;

        for (i, msg) in messages.iter().enumerate() {
            match msg.role {
                Role::Assistant => {
                    if let Some(tool_calls) = &msg.tool_calls {
                        if !tool_calls.is_empty() {
                            expecting_tool_results = true;
                            tool_calls_count = tool_calls.len();
                        } else {
                            expecting_tool_results = false;
                        }
                    } else {
                        expecting_tool_results = false;
                    }
                }
                Role::Tool => {
                    if !expecting_tool_results {
                        return Err(anyhow!(
                            "Orphaned tool result at position {} (tool_call_id: {:?})",
                            i,
                            msg.tool_call_id
                        ));
                    }
                    tool_calls_count -= 1;
                    if tool_calls_count == 0 {
                        expecting_tool_results = false;
                    }
                }
                Role::User => {
                    if expecting_tool_results {
                        return Err(anyhow!(
                            "Incomplete tool sandwich at position {} (missing {} results)",
                            i,
                            tool_calls_count
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCall;

    #[test]
    fn test_valid_tool_sandwich() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Question".to_string(),
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
                content: "Result".to_string(),
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

        assert!(MessageValidator::validate_tool_sandwich(&messages).is_ok());
    }

    #[test]
    fn test_orphaned_tool_result() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Question".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Tool,
                content: "Orphaned result".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
        ];

        let result = MessageValidator::validate_tool_sandwich(&messages);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Orphaned"));
    }

    #[test]
    fn test_incomplete_tool_sandwich() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Question".to_string(),
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
                role: Role::User,
                content: "Next question".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let result = MessageValidator::validate_tool_sandwich(&messages);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Incomplete"));
    }

    #[test]
    fn test_multiple_tool_calls() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Question".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Calling tools".to_string(),
                tool_call_id: None,
                tool_calls: Some(vec![
                    ToolCall {
                        id: "call_1".to_string(),
                        name: "tool_1".to_string(),
                        arguments: serde_json::json!({}),
                    },
                    ToolCall {
                        id: "call_2".to_string(),
                        name: "tool_2".to_string(),
                        arguments: serde_json::json!({}),
                    },
                ]),
            },
            Message {
                role: Role::Tool,
                content: "Result 1".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
            Message {
                role: Role::Tool,
                content: "Result 2".to_string(),
                tool_call_id: Some("call_2".to_string()),
                tool_calls: None,
            },
            Message {
                role: Role::Assistant,
                content: "Response".to_string(),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        assert!(MessageValidator::validate_tool_sandwich(&messages).is_ok());
    }
}
