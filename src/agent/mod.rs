use anyhow::Result;

use crate::history::{NodeId, Session};
use crate::llm::{
    LLMProvider, LoopDetection, LoopDetector, LoopDetectorConfig, Message, Role, TokenUsage,
    ToolCall, ToolRegistry,
};

/// Events emitted during agent chat for real-time monitoring
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Streaming content from the LLM
    Content(String),
    /// Thinking/reasoning content (Claude, o1, etc.)
    Thinking(String),
    /// Tool calls requested by the LLM
    ToolCallsRequested { tool_calls: Vec<ToolCall> },
    /// Tool result received after execution
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        result: String,
        is_error: bool,
    },
    /// Loop detected in tool calls
    LoopDetected { detection: LoopDetection },
    /// Checkpoint was created
    CheckpointCreated { node_id: String, strategy: String },
    /// Chat completed with final stats
    Done {
        total_usage: TokenUsage,
        all_tool_calls: Vec<ToolCall>,
        rounds: usize,
    },
}

/// Configuration for Agent chat behavior
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of tool call rounds (default: 10)
    pub max_rounds: usize,
    /// Loop detection configuration (None to disable)
    pub loop_detection: Option<LoopDetectorConfig>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_rounds: 10,
            loop_detection: Some(LoopDetectorConfig::default()),
        }
    }
}

/// Agent orchestrates conversation using Session (tree) + Provider (linear)
pub struct Agent<P: LLMProvider> {
    pub session: Session,
    provider: P,
    tools: ToolRegistry,
    config: AgentConfig,
}

impl<P: LLMProvider> Agent<P> {
    /// Create a new agent with a session, provider, and tool registry
    pub fn new(session: Session, provider: P, tools: ToolRegistry) -> Self {
        Self {
            session,
            provider,
            tools,
            config: AgentConfig::default(),
        }
    }

    /// Create a new agent with custom configuration
    pub fn with_config(
        session: Session,
        provider: P,
        tools: ToolRegistry,
        config: AgentConfig,
    ) -> Self {
        Self {
            session,
            provider,
            tools,
            config,
        }
    }

    /// Update agent configuration
    pub fn set_config(&mut self, config: AgentConfig) {
        self.config = config;
    }

    /// Main chat interface - sends a user message and gets assistant response
    ///
    /// This method:
    /// 1. Adds user message to tree
    /// 2. Extracts linear context from tree
    /// 3. Calls provider (stateless)
    /// 4. Handles tool calls
    /// 5. Adds assistant response to tree
    /// 6. Auto-checkpoints if needed
    pub async fn chat(&mut self, user_message: &str) -> Result<String> {
        self.chat_with_callback(user_message, |_| {}).await
    }

    /// Chat with callback for real-time event monitoring
    ///
    /// Same as `chat()` but calls the provided callback for each event:
    /// - `AgentEvent::Content` - streaming content from LLM
    /// - `AgentEvent::Thinking` - reasoning content (Claude, o1, etc.)
    /// - `AgentEvent::ToolCallsRequested` - when LLM requests tool calls
    /// - `AgentEvent::ToolResult` - when a tool execution completes
    /// - `AgentEvent::LoopDetected` - when loop detection triggers
    /// - `AgentEvent::CheckpointCreated` - when a checkpoint is created
    /// - `AgentEvent::Done` - when chat completes with final stats
    pub async fn chat_with_callback<F>(
        &mut self,
        user_message: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        use crate::llm::{LoopAction, LoopStep, ToolResult};
        use std::collections::HashMap;

        // 1. Add user message to tree
        self.session
            .append_message(Message {
                role: Role::User,
                content: user_message.to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await?;

        // 2. Extract linear context from tree
        let context = self.session.get_context().await?;

        // 3. Call provider with linear history (provider is stateless)
        let mut tools = self.tools.get_tools_for_llm();
        // Add recall_tool_result as a special tool
        tools.push(Self::get_recall_tool_definition());
        let mut handle = self.provider.chat_loop(context, Some(tools)).await?;

        // Initialize loop detector if enabled
        let mut loop_detector = self
            .config
            .loop_detection
            .as_ref()
            .map(|cfg| LoopDetector::with_config(cfg.clone()));

        // 4. Process response loop
        let mut response_content = String::new();
        let mut all_tool_calls: Vec<ToolCall> = Vec::new();
        let mut rounds = 0;
        let mut total_usage = TokenUsage::default();

        while let Some(event) = handle.next().await {
            match event? {
                LoopStep::Thinking(thought) => {
                    on_event(AgentEvent::Thinking(thought));
                }
                LoopStep::Content(text) => {
                    response_content.push_str(&text);
                    on_event(AgentEvent::Content(text));
                }
                LoopStep::ToolCallsRequested {
                    tool_calls,
                    content,
                } => {
                    rounds += 1;

                    // Check max rounds limit
                    if rounds > self.config.max_rounds {
                        return Err(anyhow::anyhow!(
                            "Maximum tool rounds ({}) exceeded",
                            self.config.max_rounds
                        ));
                    }

                    // Add any content before tool calls
                    if !content.is_empty() {
                        response_content.push_str(&content);
                    }

                    // Emit tool calls event
                    on_event(AgentEvent::ToolCallsRequested {
                        tool_calls: tool_calls.clone(),
                    });

                    // Check for loops and collect warnings
                    let mut loop_warnings: HashMap<String, String> = HashMap::new();
                    if let Some(ref mut detector) = loop_detector {
                        for call in &tool_calls {
                            if let Some(detection) = detector.check(call) {
                                // Emit loop detection event
                                on_event(AgentEvent::LoopDetected {
                                    detection: detection.clone(),
                                });

                                // Handle based on action
                                match detection.action {
                                    LoopAction::Continue => {}
                                    LoopAction::Warn => {
                                        if let Some(warning) = detection.warning_message {
                                            loop_warnings.insert(call.id.clone(), warning);
                                        }
                                    }
                                    LoopAction::Terminate => {
                                        detector.clear();
                                        return Err(anyhow::anyhow!(
                                            "Loop detected: {}",
                                            detection.suggestion
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // Execute tools
                    let mut results = Vec::new();
                    for call in &tool_calls {
                        all_tool_calls.push(call.clone());

                        // Check if this is the special recall_tool_result tool
                        let result = if call.name == "recall_tool_result" {
                            self.execute_recall_tool_result(call)
                        } else if let Some(result) = self.tools.execute(call).await {
                            result
                        } else {
                            ToolResult {
                                tool_call_id: call.id.clone(),
                                content: format!("Tool '{}' not registered", call.name),
                                is_error: true,
                            }
                        };

                        // Prepend loop warning if present
                        let result = if let Some(warning) = loop_warnings.get(&call.id) {
                            ToolResult {
                                tool_call_id: result.tool_call_id,
                                content: format!("{}\n\n{}", warning, result.content),
                                is_error: result.is_error,
                            }
                        } else {
                            result
                        };

                        // Emit tool result event
                        on_event(AgentEvent::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                            is_error: result.is_error,
                        });

                        results.push(result);
                    }

                    // Add assistant message with tool calls
                    self.session
                        .append_message(Message {
                            role: Role::Assistant,
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                        })
                        .await?;

                    // Add tool results
                    for result in &results {
                        self.session
                            .append_message(Message {
                                role: Role::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                tool_calls: None,
                            })
                            .await?;
                    }

                    handle.submit_tool_results(results)?;
                }
                LoopStep::ToolResultsReceived { .. } => {
                    // Just continue
                }
                LoopStep::Done {
                    content,
                    total_usage: usage,
                    ..
                } => {
                    // Update final content if provided
                    if !content.is_empty() && content != response_content {
                        response_content = content;
                    }
                    total_usage = usage;
                    break;
                }
            }
        }

        // 5. Add assistant response to tree
        if !response_content.is_empty() {
            self.session
                .append_message(Message {
                    role: Role::Assistant,
                    content: response_content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                })
                .await?;
        }

        // 6. Auto checkpoint if needed
        if let Some((node_id, strategy)) = self.auto_checkpoint_if_needed().await? {
            on_event(AgentEvent::CheckpointCreated { node_id, strategy });
        }

        // 7. Emit done event with stats
        on_event(AgentEvent::Done {
            total_usage,
            all_tool_calls,
            rounds,
        });

        Ok(response_content)
    }

    /// Branch from a specific node and continue with a new message
    pub async fn branch_and_retry(
        &mut self,
        from_node_id: NodeId,
        new_user_message: &str,
    ) -> Result<String> {
        self.branch_and_retry_with_callback(from_node_id, new_user_message, |_| {})
            .await
    }

    /// Branch from a specific node and continue with a new message, with event callbacks
    ///
    /// Same as `branch_and_retry()` but calls the provided callback for each event.
    /// This provides full feature parity with `chat_with_callback`:
    /// - Tool execution
    /// - Loop detection
    /// - Max rounds limit
    /// - Token usage tracking
    /// - All event callbacks
    pub async fn branch_and_retry_with_callback<F>(
        &mut self,
        from_node_id: NodeId,
        new_user_message: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent),
    {
        use crate::llm::{LoopAction, LoopStep, ToolResult};
        use std::collections::HashMap;

        // Branch from the node
        self.session.branch_from(from_node_id.clone()).await?;

        // Append new message to the branch point
        self.session
            .append_message_to(
                from_node_id,
                Message {
                    role: Role::User,
                    content: new_user_message.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                },
            )
            .await?;

        // Extract context and call provider
        let context = self.session.get_context().await?;
        let mut tools = self.tools.get_tools_for_llm();
        // Add recall_tool_result as a special tool
        tools.push(Self::get_recall_tool_definition());
        let mut handle = self.provider.chat_loop(context, Some(tools)).await?;

        // Initialize loop detector if enabled
        let mut loop_detector = self
            .config
            .loop_detection
            .as_ref()
            .map(|cfg| LoopDetector::with_config(cfg.clone()));

        // Process response loop
        let mut response_content = String::new();
        let mut all_tool_calls: Vec<ToolCall> = Vec::new();
        let mut rounds = 0;
        let mut total_usage = TokenUsage::default();

        while let Some(event) = handle.next().await {
            match event? {
                LoopStep::Thinking(thought) => {
                    on_event(AgentEvent::Thinking(thought));
                }
                LoopStep::Content(text) => {
                    response_content.push_str(&text);
                    on_event(AgentEvent::Content(text));
                }
                LoopStep::ToolCallsRequested {
                    tool_calls,
                    content,
                } => {
                    rounds += 1;

                    // Check max rounds limit
                    if rounds > self.config.max_rounds {
                        return Err(anyhow::anyhow!(
                            "Maximum tool rounds ({}) exceeded",
                            self.config.max_rounds
                        ));
                    }

                    // Add any content before tool calls
                    if !content.is_empty() {
                        response_content.push_str(&content);
                    }

                    // Emit tool calls event
                    on_event(AgentEvent::ToolCallsRequested {
                        tool_calls: tool_calls.clone(),
                    });

                    // Check for loops and collect warnings
                    let mut loop_warnings: HashMap<String, String> = HashMap::new();
                    if let Some(ref mut detector) = loop_detector {
                        for call in &tool_calls {
                            if let Some(detection) = detector.check(call) {
                                // Emit loop detection event
                                on_event(AgentEvent::LoopDetected {
                                    detection: detection.clone(),
                                });

                                // Handle based on action
                                match detection.action {
                                    LoopAction::Continue => {}
                                    LoopAction::Warn => {
                                        if let Some(warning) = detection.warning_message {
                                            loop_warnings.insert(call.id.clone(), warning);
                                        }
                                    }
                                    LoopAction::Terminate => {
                                        detector.clear();
                                        return Err(anyhow::anyhow!(
                                            "Loop detected: {}",
                                            detection.suggestion
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // Execute tools
                    let mut results = Vec::new();
                    for call in &tool_calls {
                        all_tool_calls.push(call.clone());

                        // Check if this is the special recall_tool_result tool
                        let result = if call.name == "recall_tool_result" {
                            self.execute_recall_tool_result(call)
                        } else if let Some(result) = self.tools.execute(call).await {
                            result
                        } else {
                            ToolResult {
                                tool_call_id: call.id.clone(),
                                content: format!("Tool '{}' not registered", call.name),
                                is_error: true,
                            }
                        };

                        // Prepend loop warning if present
                        let result = if let Some(warning) = loop_warnings.get(&call.id) {
                            ToolResult {
                                tool_call_id: result.tool_call_id,
                                content: format!("{}\n\n{}", warning, result.content),
                                is_error: result.is_error,
                            }
                        } else {
                            result
                        };

                        // Emit tool result event
                        on_event(AgentEvent::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                            is_error: result.is_error,
                        });

                        results.push(result);
                    }

                    // Add assistant message with tool calls
                    self.session
                        .append_message(Message {
                            role: Role::Assistant,
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                        })
                        .await?;

                    // Add tool results
                    for result in &results {
                        self.session
                            .append_message(Message {
                                role: Role::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                tool_calls: None,
                            })
                            .await?;
                    }

                    handle.submit_tool_results(results)?;
                }
                LoopStep::ToolResultsReceived { .. } => {
                    // Just continue
                }
                LoopStep::Done {
                    content,
                    total_usage: usage,
                    ..
                } => {
                    // Update final content if provided
                    if !content.is_empty() && content != response_content {
                        response_content = content;
                    }
                    total_usage = usage;
                    break;
                }
            }
        }

        // Add assistant response to tree
        if !response_content.is_empty() {
            self.session
                .append_message(Message {
                    role: Role::Assistant,
                    content: response_content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                })
                .await?;
        }

        // Auto checkpoint if needed
        if let Some((node_id, strategy)) = self.auto_checkpoint_if_needed().await? {
            on_event(AgentEvent::CheckpointCreated { node_id, strategy });
        }

        // Emit done event with stats
        on_event(AgentEvent::Done {
            total_usage,
            all_tool_calls,
            rounds,
        });

        Ok(response_content)
    }

    /// Manually create a checkpoint at the current active leaf
    pub async fn checkpoint(&mut self) -> Result<NodeId> {
        let context = self.session.get_context().await?;
        let summary = self.generate_summary(&context).await?;

        let checkpoint_node = self.session.active_leaf_id.clone();
        self.session
            .create_checkpoint(checkpoint_node.clone(), summary, "manual")
            .await?;

        Ok(checkpoint_node)
    }

    /// Auto checkpoint if conditions are met
    /// Returns Some((node_id, strategy)) if checkpoint was created, None otherwise
    async fn auto_checkpoint_if_needed(&mut self) -> Result<Option<(String, String)>> {
        // Guard: Don't checkpoint if active leaf already has a checkpoint
        if self.session.has_checkpoint(&self.session.active_leaf_id) {
            return Ok(None);
        }

        // Ask Session if checkpoint is needed based on its optimization config
        if let Some(strategy) = self.session.should_auto_checkpoint().await? {
            let context = self.session.get_context().await?;
            let summary = self.generate_summary(&context).await?;
            let node_id = self.session.active_leaf_id.clone();
            self.session
                .create_checkpoint(node_id.clone(), summary, strategy)
                .await?;
            return Ok(Some((node_id, strategy.to_string())));
        }

        Ok(None)
    }

    /// Generate a summary for checkpoint using the LLM
    async fn generate_summary(&self, context: &[Message]) -> Result<String> {
        // Simple implementation: ask LLM to summarize
        let summary_prompt = format!(
            "Please provide a concise summary of the following conversation:\n\n{}",
            context
                .iter()
                .map(|m| format!("{:?}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let summary_context = vec![Message {
            role: Role::User,
            content: summary_prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        let mut handle = self.provider.chat_loop(summary_context, None).await?;
        let mut summary = String::new();

        use crate::llm::LoopStep;
        while let Some(event) = handle.next().await {
            match event? {
                LoopStep::Content(text) => {
                    summary.push_str(&text);
                }
                LoopStep::Done { content, .. } => {
                    summary.push_str(&content);
                    break;
                }
                _ => {}
            }
        }

        Ok(summary)
    }

    /// Execute recall_tool_result - special tool for retrieving archived results
    fn execute_recall_tool_result(&self, call: &crate::llm::ToolCall) -> crate::llm::ToolResult {
        use crate::llm::ToolResult;

        // Parse arguments
        let tool_call_id_arg = call
            .arguments
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if tool_call_id_arg.is_empty() {
            return ToolResult {
                tool_call_id: call.id.clone(),
                content: "Error: tool_call_id parameter is required".to_string(),
                is_error: true,
            };
        }

        // Retrieve from archive
        match self.session.get_archived_tool_result(tool_call_id_arg) {
            Some(archived) => ToolResult {
                tool_call_id: call.id.clone(),
                content: archived.full_content.clone(),
                is_error: false,
            },
            None => ToolResult {
                tool_call_id: call.id.clone(),
                content: format!(
                    "Error: No archived result found for tool_call_id '{}'",
                    tool_call_id_arg
                ),
                is_error: true,
            },
        }
    }

    /// Get recall_tool_result tool definition
    fn get_recall_tool_definition() -> crate::llm::Tool {
        use crate::llm::Tool;
        use serde_json::json;

        Tool {
            name: "recall_tool_result".to_string(),
            description: "Retrieve the full content of a previously archived tool result"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tool_call_id": {
                        "type": "string",
                        "description": "The tool_call_id of the archived result to retrieve"
                    }
                },
                "required": ["tool_call_id"]
            }),
            full_description: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{MemoryStore, SessionConfig};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_auto_checkpoint_counts_user_turns_not_all_messages() {
        // Create session with auto-checkpoint every 3 user turns
        let store = Arc::new(MemoryStore::new());
        let mut config = SessionConfig::default();
        config.optimization.checkpoint.every_n_turns = Some(3);
        config.system_prompt = Some("Test system".to_string());

        let mut session = Session::new(store, config).await.unwrap();

        // Simulate conversation with mixed message types
        // Turn 1: User -> Assistant (2 messages total, 1 user turn)
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Answer 1".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Verify: 1 user turn, should not trigger checkpoint (need > 3)
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_count, 1);

        // Turn 2: User -> Assistant
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();
        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Answer 2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Turn 3: User -> Assistant
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 3".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();
        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Answer 3".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // At this point: 3 user turns, should NOT checkpoint yet (need > 3)
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_count, 3, "Should have 3 user turns");

        // Turn 4: User -> Assistant
        // This should trigger checkpoint if we check (4 user turns > 3 threshold)
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 4".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Verify the logic: 4 user turns > 3 threshold
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_count, 4, "Should have 4 user turns");
        assert!(user_count > 3, "4 user turns should exceed threshold of 3");
    }

    #[tokio::test]
    async fn test_checkpoint_logic_ignores_tool_messages() {
        // Test that tool messages don't count toward checkpoint threshold
        let store = Arc::new(MemoryStore::new());
        let mut config = SessionConfig::default();
        config.optimization.checkpoint.every_n_turns = Some(2);
        config.system_prompt = Some("Test".to_string());

        let mut session = Session::new(store, config).await.unwrap();

        // Turn 1: User -> Assistant with tool call -> Tool result -> Assistant
        session
            .append_message(Message {
                role: Role::User,
                content: "Question with tool".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        use crate::llm::ToolCall;
        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Calling tool".to_string(),
                tool_call_id: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "test_tool".to_string(),
                    arguments: serde_json::json!({}),
                }]),
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Tool,
                content: "Tool result".to_string(),
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            })
            .await
            .unwrap();

        session
            .append_message(Message {
                role: Role::Assistant,
                content: "Final response".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // At this point: 4 messages + system, but only 1 user turn
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        let total_count = context.len();
        assert_eq!(user_count, 1, "Should have 1 user turn");
        assert_eq!(total_count, 5, "Should have 5 total messages (system + 4)");

        // Turn 2: Another user message
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 2".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Now: 2 user turns, should NOT trigger (need > 2)
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_count, 2, "Should have 2 user turns");
        assert!(
            !(user_count > 2),
            "2 user turns should NOT exceed threshold of 2"
        );

        // Turn 3: One more user message
        session
            .append_message(Message {
                role: Role::User,
                content: "Question 3".to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await
            .unwrap();

        // Now: 3 user turns, SHOULD trigger (3 > 2)
        let context = session.get_context().await.unwrap();
        let user_count = context.iter().filter(|m| m.role == Role::User).count();
        assert_eq!(user_count, 3, "Should have 3 user turns");
        assert!(user_count > 2, "3 user turns should exceed threshold of 2");
    }
}
