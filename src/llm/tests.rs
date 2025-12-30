#[cfg(test)]
mod tests {
    use super::super::provider::*;

    #[test]
    fn test_provider_state_default() {
        let state = ProviderState::default();
        assert_eq!(state.input_tokens, 0);
        assert_eq!(state.output_tokens, 0);
        assert_eq!(state.cached_tokens, 0);
    }

    #[test]
    fn test_provider_config_default() {
        let config = ProviderConfig::default();
        assert_eq!(config.temperature, 1.0);
        assert_eq!(config.max_tokens, 40960);
        assert_eq!(config.enable_reasoning, false);
        assert_eq!(config.max_tool_turns, Some(3));
    }

    #[test]
    fn test_message_creation() {
        let msg = Message {
            role: Role::User,
            content: "Hello".to_string(),
            tool_call_id: None,
            tool_calls: None,
        };
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_tool_call_assembler() {
        let mut assembler = ToolCallAssembler::new();

        // Simulate receiving tool call deltas
        assembler.process_delta(
            "call_1".to_string(),
            Some("get_weather".to_string()),
            Some(r#"{"city": "#.to_string()),
        );

        assembler.process_delta("call_1".to_string(), None, Some(r#""Tokyo"}"#.to_string()));

        let tool_calls = assembler.into_tool_calls().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].name, "get_weather");
        assert_eq!(tool_calls[0].arguments["city"], "Tokyo");
    }

    #[test]
    fn test_parallel_tool_calls() {
        let mut assembler = ToolCallAssembler::new();

        // Simulate two parallel tool calls
        assembler.process_delta(
            "call_1".to_string(),
            Some("get_weather".to_string()),
            Some(r#"{"city": "Tokyo"}"#.to_string()),
        );

        assembler.process_delta(
            "call_2".to_string(),
            Some("get_weather".to_string()),
            Some(r#"{"city": "Paris"}"#.to_string()),
        );

        let tool_calls = assembler.into_tool_calls().unwrap();
        assert_eq!(tool_calls.len(), 2);

        // Find both calls
        let tokyo_call = tool_calls.iter().find(|c| c.id == "call_1").unwrap();
        let paris_call = tool_calls.iter().find(|c| c.id == "call_2").unwrap();

        assert_eq!(tokyo_call.arguments["city"], "Tokyo");
        assert_eq!(paris_call.arguments["city"], "Paris");
    }

    #[test]
    fn test_tool_result_creation() {
        let result = ToolResult {
            tool_call_id: "call_1".to_string(),
            content: "Weather is sunny".to_string(),
            is_error: false,
        };

        assert_eq!(result.tool_call_id, "call_1");
        assert!(!result.is_error);
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cached_tokens, 0);
    }

    #[test]
    fn test_finish_reason_equality() {
        assert_eq!(FinishReason::Stop, FinishReason::Stop);
        assert_ne!(FinishReason::Stop, FinishReason::Length);
        assert_eq!(
            FinishReason::Other("custom".to_string()),
            FinishReason::Other("custom".to_string())
        );
    }
}
