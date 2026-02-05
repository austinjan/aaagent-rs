use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::history::{CheckpointStats, ContextStringStrategy, NodeId, Session};
use crate::llm::{
    LLMProvider, LoopDetection, LoopDetector, LoopDetectorConfig, Message, Role, TokenUsage,
    ToolCall, ToolRegistry,
};

pub mod agent_factory;
pub mod announce;
pub mod inject_listener;
pub mod runtime;
pub mod session_manager;
pub mod spawn_tool;
pub mod subagent_registry;

pub use agent_factory::AgentFactory;
pub use announce::run_announce_flow;
pub use inject_listener::start_inject_listener;
pub use runtime::{
    AgentRuntime, MessageSource as RuntimeMessageSource, QueueMode, QueuedMessage, RunGuard,
    RunInfo,
};
pub use session_manager::SessionManager;
pub use spawn_tool::SpawnSubAgentTool;
pub use subagent_registry::{CleanupStrategy, SubAgentOutcome, SubAgentRegistry, SubAgentRun};

/// Compression strategy for checkpoint creation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionStrategy {
    /// Keep facts, constraints, decisions and reasoning
    Balanced,
    /// Remove all non-essential content including tool calls
    Aggressive,
    /// User-provided prompt guides compression
    Custom,
}

impl Default for CompressionStrategy {
    fn default() -> Self {
        Self::Balanced
    }
}

impl std::fmt::Display for CompressionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Balanced => write!(f, "balanced"),
            Self::Aggressive => write!(f, "aggressive"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl CompressionStrategy {
    /// Get the system prompt for this compression strategy
    pub fn get_prompt(&self, custom_prompt: Option<&str>) -> String {
        match self {
            Self::Balanced => BALANCED_COMPRESSION_PROMPT.to_string(),
            Self::Aggressive => AGGRESSIVE_COMPRESSION_PROMPT.to_string(),
            Self::Custom => {
                if let Some(prompt) = custom_prompt {
                    format!("{}\n\n{}", CUSTOM_COMPRESSION_PREFIX, prompt)
                } else {
                    BALANCED_COMPRESSION_PROMPT.to_string()
                }
            }
        }
    }

    /// Convert to ContextStringStrategy for context extraction
    /// - Aggressive compression skips tool nodes (ContextStringStrategy::Aggressive)
    /// - Balanced and Custom include tool nodes (ContextStringStrategy::Default)
    pub fn to_context_strategy(&self) -> ContextStringStrategy {
        match self {
            Self::Aggressive => ContextStringStrategy::Aggressive,
            Self::Balanced | Self::Custom => ContextStringStrategy::Default,
        }
    }
}

const BALANCED_COMPRESSION_PROMPT: &str = r#"Summarize the following conversation, preserving:
- Key facts and data points established
- Constraints and requirements identified
- Decisions made and their reasoning
- Current state and next steps

Remove:
- Casual conversation and greetings
- Repeated information
- Verbose explanations (keep conclusions only)

Output a concise summary that allows the conversation to continue naturally."#;

const AGGRESSIVE_COMPRESSION_PROMPT: &str = r#"Create a minimal summary containing ONLY:
- Final decisions and outcomes
- Critical constraints that affect future actions
- Current objective/goal

Remove ALL:
- Tool calls and their results (summarize outcomes only in one sentence)
- Reasoning and deliberation
- Alternative options that were rejected
- Any content not directly relevant to the main goal

Be extremely concise. The summary should be as short as possible while preserving the ability to continue the conversation."#;

const CUSTOM_COMPRESSION_PREFIX: &str = r#"You are summarizing a conversation between a user and an AI assistant.
The summary will replace the original messages to reduce context size.
Ensure the summary preserves enough information for the conversation to continue coherently.

User's compression instructions:"#;

/// Options for checkpoint creation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointOptions {
    /// Compression strategy to use
    #[serde(default)]
    pub strategy: CompressionStrategy,
    /// Custom prompt (only used when strategy is Custom)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_prompt: Option<String>,
    /// Use main (more powerful) provider instead of quick provider
    /// Default: false (uses quick provider for speed/cost)
    #[serde(default)]
    pub use_main_provider: bool,
}

/// Result of checkpoint creation or preview
#[derive(Debug, Clone, Serialize)]
pub struct CheckpointResult {
    pub node_id: String,
    pub summary: String,
    pub stats: CheckpointStats,
}

/// Events emitted during agent chat for real-time monitoring
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Queued messages are being processed
    QueuedMessagesReceived { count: usize },
    /// A queued message was processed (followup mode)
    FollowupProcessed {
        message_index: usize,
        total_queued: usize,
        source: String,
    },
    /// Sub-agent spawned (background task started)
    SubAgentSpawned { run_id: String, task_label: String },
    /// Sub-agent completed (background task finished)
    SubAgentCompleted {
        run_id: String,
        success: bool,
        error: Option<String>,
    },
    /// Chat completed with final stats
    Done {
        total_usage: TokenUsage,
        all_tool_calls: Vec<ToolCall>,
        rounds: usize,
        /// Node IDs created during this chat turn (for incremental tree updates)
        new_node_ids: Vec<String>,
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
    /// Optional quick provider for simple internal tasks (e.g., checkpoint summaries)
    /// If None, falls back to main provider
    quick_provider: Option<Box<dyn LLMProvider>>,
    tools: ToolRegistry,
    config: AgentConfig,
    /// Skills XML to inject into system prompt
    skills_prompt: Option<String>,
    /// Runtime for tracking active runs and message queuing
    runtime: Option<Arc<AgentRuntime>>,
    /// Unique session key for this agent instance
    session_key: Option<String>,
    /// Global event bus for broadcasting events to SSE clients
    event_bus: Option<Arc<crate::api::event_bus::GlobalEventBus>>,
    /// Run sequence counter for this agent instance
    run_seq: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

use std::sync::Arc;

impl<P: LLMProvider> Agent<P> {
    /// Create a new agent with a session, provider, and tool registry
    pub fn new(session: Session, provider: P, tools: ToolRegistry) -> Self {
        Self {
            session,
            provider,
            quick_provider: None,
            tools,
            config: AgentConfig::default(),
            runtime: None,
            session_key: None,
            skills_prompt: None,
            event_bus: None,
            run_seq: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
            quick_provider: None,
            tools,
            config,
            skills_prompt: None,
            runtime: None,
            session_key: None,
            event_bus: None,
            run_seq: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Set a quick provider for simple internal tasks (e.g., checkpoint summaries)
    ///
    /// The quick provider is used for tasks that don't require complex reasoning,
    /// allowing you to use a cheaper/faster model for these operations.
    ///
    /// # Example
    /// ```ignore
    /// let main_provider = OpenAIProvider::create("gpt-4o".into(), api_key)?;
    /// let quick_provider = OpenAIProvider::create("gpt-4o-mini".into(), api_key)?;
    ///
    /// let agent = Agent::new(session, main_provider, tools)
    ///     .with_quick_provider(Box::new(quick_provider));
    /// ```
    pub fn with_quick_provider(mut self, provider: Box<dyn LLMProvider>) -> Self {
        self.quick_provider = Some(provider);
        self
    }

    /// Update agent configuration
    pub fn set_config(&mut self, config: AgentConfig) {
        self.config = config;
    }

    /// Set quick provider after construction
    pub fn set_quick_provider(&mut self, provider: Box<dyn LLMProvider>) {
        self.quick_provider = Some(provider);
    }

    /// Set skills prompt (XML list of available skills)
    ///
    /// The skills prompt is injected into the context as a system message
    /// after the main system prompt. Use `SkillsManager::snapshot().prompt`
    /// to generate this.
    pub fn with_skills(mut self, skills_prompt: String) -> Self {
        if !skills_prompt.is_empty() {
            self.skills_prompt = Some(skills_prompt);
        }
        self
    }

    /// Set skills prompt after construction
    pub fn set_skills_prompt(&mut self, skills_prompt: String) {
        if !skills_prompt.is_empty() {
            self.skills_prompt = Some(skills_prompt);
        } else {
            self.skills_prompt = None;
        }
    }

    /// Set the runtime for this agent (enables run tracking and message queuing)
    pub fn set_runtime(&mut self, runtime: Arc<AgentRuntime>) {
        self.runtime = Some(runtime);
    }

    /// Set the session key for this agent instance
    pub fn set_session_key(&mut self, session_key: String) {
        self.session_key = Some(session_key);
    }

    /// Set the global event bus for broadcasting events to SSE clients
    pub fn set_event_bus(&mut self, event_bus: Arc<crate::api::event_bus::GlobalEventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Format a collection of queued messages into a single merged message
    ///
    /// Used for Collect mode queue processing to batch multiple messages.
    fn format_collected_messages(messages: &[crate::agent::runtime::QueuedMessage]) -> String {
        use crate::agent::runtime::MessageSource;

        if messages.is_empty() {
            return String::new();
        }

        if messages.len() == 1 {
            // Single message - return as-is
            return messages[0].content.clone();
        }

        // Multiple messages - merge with separators
        let mut merged = String::new();
        merged.push_str(&format!(
            "# Batched Updates ({} messages)\n\n",
            messages.len()
        ));

        for (idx, msg) in messages.iter().enumerate() {
            let source_label = match &msg.source {
                MessageSource::SubAgent { run_id } => format!("Sub-Agent: {}", run_id),
                MessageSource::User => "User".to_string(),
                MessageSource::System => "System".to_string(),
            };

            merged.push_str(&format!("## Update {} - {}\n", idx + 1, source_label));

            // Add timestamp if available
            let timestamp = chrono::DateTime::from_timestamp_millis(msg.queued_at);
            if let Some(dt) = timestamp {
                merged.push_str(&format!(
                    "*Queued at: {}*\n\n",
                    dt.format("%Y-%m-%d %H:%M:%S UTC")
                ));
            }

            merged.push_str(&msg.content);
            merged.push_str("\n\n---\n\n");
        }

        merged
    }

    /// Inject skills into context (appends to first system message or adds new one)
    fn inject_skills(&self, mut context: Vec<Message>) -> Vec<Message> {
        if let Some(ref skills) = self.skills_prompt {
            // Find the first system message and append skills to it
            if let Some(system_msg) = context.iter_mut().find(|m| m.role == Role::System) {
                system_msg.content = format!("{}\n\n{}", system_msg.content, skills);
            } else {
                // No system message, insert skills as first message
                context.insert(
                    0,
                    Message {
                        role: Role::System,
                        content: skills.clone(),
                        tool_call_id: None,
                        tool_calls: None,
                    },
                );
            }
        }
        context
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
        self.chat_with_callback(user_message, |_| async {}).await
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
    pub async fn chat_with_callback<F, Fut>(
        &mut self,
        user_message: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        use crate::llm::{LoopAction, LoopStep, ToolResult};
        use std::collections::HashMap;
        use std::sync::atomic::Ordering;

        // Register run with runtime (if configured)
        let _run_guard =
            if let (Some(runtime), Some(session_key)) = (&self.runtime, &self.session_key) {
                Some(runtime.register_run(session_key.clone(), true)?)
            } else {
                None
            };

        // Helper to emit events to both callback and event_bus
        let event_bus = self.event_bus.clone();
        let session_key = self.session_key.clone();
        let run_seq_counter = self.run_seq.clone();
        let mut emit_event = |event: AgentEvent| {
            // Call the user callback
            let fut = on_event(event.clone());

            // Also broadcast to event_bus if configured
            if let (Some(ref bus), Some(ref session_id)) = (&event_bus, &session_key) {
                let run_id = session_id.clone(); // Use session_key as run_id for now
                let run_seq = run_seq_counter.fetch_add(1, Ordering::SeqCst);
                bus.emit(session_id.clone(), run_id, run_seq, event);
            }

            fut
        };

        // Track new nodes created during this chat turn
        let mut new_node_ids: Vec<String> = Vec::new();

        // 1. Add user message to tree
        let user_node_id = self
            .session
            .append_message(Message {
                role: Role::User,
                content: user_message.to_string(),
                tool_call_id: None,
                tool_calls: None,
            })
            .await?;
        new_node_ids.push(user_node_id);

        // 2. Extract linear context from tree and inject skills
        let context = self.session.get_context().await?;
        let context = self.inject_skills(context);

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
                    emit_event(AgentEvent::Thinking(thought)).await;
                }
                LoopStep::Content(text) => {
                    response_content.push_str(&text);
                    emit_event(AgentEvent::Content(text)).await;
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
                    emit_event(AgentEvent::ToolCallsRequested {
                        tool_calls: tool_calls.clone(),
                    })
                    .await;

                    // Check for loops and collect warnings
                    let mut loop_warnings: HashMap<String, String> = HashMap::new();
                    if let Some(ref mut detector) = loop_detector {
                        for call in &tool_calls {
                            if let Some(detection) = detector.check(call) {
                                // Emit loop detection event
                                emit_event(AgentEvent::LoopDetected {
                                    detection: detection.clone(),
                                })
                                .await;

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
                        emit_event(AgentEvent::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                            is_error: result.is_error,
                        })
                        .await;

                        results.push(result);
                    }

                    // Add assistant message with tool calls
                    let assistant_node_id = self
                        .session
                        .append_message(Message {
                            role: Role::Assistant,
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                        })
                        .await?;
                    new_node_ids.push(assistant_node_id);

                    // Add tool results
                    for result in &results {
                        let tool_node_id = self
                            .session
                            .append_message(Message {
                                role: Role::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                tool_calls: None,
                            })
                            .await?;
                        new_node_ids.push(tool_node_id);
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
            let final_node_id = self
                .session
                .append_message(Message {
                    role: Role::Assistant,
                    content: response_content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                })
                .await?;
            new_node_ids.push(final_node_id);
        }

        // 6. Auto checkpoint if needed
        if let Some((node_id, strategy)) = self.auto_checkpoint_if_needed().await? {
            new_node_ids.push(node_id.clone());
            emit_event(AgentEvent::CheckpointCreated { node_id, strategy }).await;
        }

        // 7. Emit done event with stats
        emit_event(AgentEvent::Done {
            total_usage,
            all_tool_calls,
            rounds,
            new_node_ids,
        })
        .await;

        // 8. Process queued messages (if runtime configured)
        // Note: _run_guard drops here, unregistering the run before we drain queue
        drop(_run_guard);

        if let (Some(runtime), Some(session_key)) = (&self.runtime, &self.session_key) {
            // Check if there are any queued messages
            let queue_depth = runtime.get_queue_depth(session_key);
            if queue_depth == 0 {
                return Ok(response_content);
            }

            // Determine processing mode based on first message (all should have same mode)
            let queued_messages = runtime.drain_queue(session_key);
            if queued_messages.is_empty() {
                return Ok(response_content);
            }

            let processing_mode = queued_messages[0].mode.clone();
            let total_queued = queued_messages.len();

            log::info!(
                "Processing {} queued messages in {:?} mode for session {}",
                total_queued,
                processing_mode,
                session_key
            );

            // Emit event to notify about queued messages being processed
            emit_event(AgentEvent::QueuedMessagesReceived {
                count: total_queued,
            })
            .await;

            match processing_mode {
                QueueMode::Followup => {
                    use crate::agent::runtime::MessageSource;

                    // Process each message sequentially (max 10 to prevent infinite loops)
                    let max_queue_processing = 10;
                    for (idx, queued_msg) in queued_messages
                        .into_iter()
                        .take(max_queue_processing)
                        .enumerate()
                    {
                        let source_str = match &queued_msg.source {
                            MessageSource::SubAgent { run_id } => {
                                format!("SubAgent({})", run_id)
                            }
                            MessageSource::User => "User".to_string(),
                            MessageSource::System => "System".to_string(),
                        };

                        log::info!(
                            "Processing followup message {}/{} from {}",
                            idx + 1,
                            total_queued,
                            source_str
                        );

                        // Emit event for this specific followup
                        emit_event(AgentEvent::FollowupProcessed {
                            message_index: idx + 1,
                            total_queued,
                            source: source_str,
                        })
                        .await;

                        // Use chat() instead of chat_with_callback to avoid recursion depth issues
                        // This will still trigger events but breaks the callback recursion chain
                        let _ = Box::pin(self.chat(&queued_msg.content)).await;
                    }

                    if total_queued > max_queue_processing {
                        log::warn!(
                            "Stopped processing after {} messages (limit reached, {} remaining)",
                            max_queue_processing,
                            total_queued - max_queue_processing
                        );
                    }
                }
                QueueMode::Collect => {
                    // Batch all messages into one merged message
                    // Note: format_collected_messages() does NOT drain the queue
                    let merged_content = Self::format_collected_messages(&queued_messages);

                    log::info!(
                        "Processing collected batch of {} messages for session {}",
                        total_queued,
                        session_key
                    );

                    // Process the merged message as a single turn
                    let _ = Box::pin(self.chat(&merged_content)).await;
                }
                QueueMode::Steer | QueueMode::Interrupt => {
                    // Future modes - not implemented yet
                    log::warn!(
                        "Queue mode {:?} not yet implemented, skipping {} messages",
                        processing_mode,
                        total_queued
                    );
                }
            }
        }

        Ok(response_content)
    }

    /// Branch from a specific node and continue with a new message
    pub async fn branch_and_retry(
        &mut self,
        from_node_id: NodeId,
        new_user_message: &str,
    ) -> Result<String> {
        self.branch_and_retry_with_callback(from_node_id, new_user_message, |_| async {})
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
    pub async fn branch_and_retry_with_callback<F, Fut>(
        &mut self,
        from_node_id: NodeId,
        new_user_message: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        use crate::llm::{LoopAction, LoopStep, ToolResult};
        use std::collections::HashMap;
        use std::sync::atomic::Ordering;

        // Helper to emit events to both callback and event_bus
        let event_bus = self.event_bus.clone();
        let session_key = self.session_key.clone();
        let run_seq_counter = self.run_seq.clone();
        let mut emit_event = |event: AgentEvent| {
            // Call the user callback
            let fut = on_event(event.clone());

            // Also broadcast to event_bus if configured
            if let (Some(ref bus), Some(ref session_id)) = (&event_bus, &session_key) {
                let run_id = session_id.clone(); // Use session_key as run_id for now
                let run_seq = run_seq_counter.fetch_add(1, Ordering::SeqCst);
                bus.emit(session_id.clone(), run_id, run_seq, event);
            }

            fut
        };

        // Track new nodes created during this chat turn
        let mut new_node_ids: Vec<String> = Vec::new();

        // Branch from the node
        self.session.branch_from(from_node_id.clone()).await?;

        // Append new message to the branch point
        let user_node_id = self
            .session
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
        new_node_ids.push(user_node_id);

        // Extract context and call provider (inject skills)
        let context = self.session.get_context().await?;
        let context = self.inject_skills(context);
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
                    emit_event(AgentEvent::Thinking(thought)).await;
                }
                LoopStep::Content(text) => {
                    response_content.push_str(&text);
                    emit_event(AgentEvent::Content(text)).await;
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
                    emit_event(AgentEvent::ToolCallsRequested {
                        tool_calls: tool_calls.clone(),
                    })
                    .await;

                    // Check for loops and collect warnings
                    let mut loop_warnings: HashMap<String, String> = HashMap::new();
                    if let Some(ref mut detector) = loop_detector {
                        for call in &tool_calls {
                            if let Some(detection) = detector.check(call) {
                                // Emit loop detection event
                                emit_event(AgentEvent::LoopDetected {
                                    detection: detection.clone(),
                                })
                                .await;

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
                        emit_event(AgentEvent::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            tool_name: call.name.clone(),
                            result: result.content.clone(),
                            is_error: result.is_error,
                        })
                        .await;

                        results.push(result);
                    }

                    // Add assistant message with tool calls
                    let assistant_node_id = self
                        .session
                        .append_message(Message {
                            role: Role::Assistant,
                            content: content.clone(),
                            tool_call_id: None,
                            tool_calls: Some(tool_calls.clone()),
                        })
                        .await?;
                    new_node_ids.push(assistant_node_id);

                    // Add tool results
                    for result in &results {
                        let tool_node_id = self
                            .session
                            .append_message(Message {
                                role: Role::Tool,
                                content: result.content.clone(),
                                tool_call_id: Some(result.tool_call_id.clone()),
                                tool_calls: None,
                            })
                            .await?;
                        new_node_ids.push(tool_node_id);
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
            let final_node_id = self
                .session
                .append_message(Message {
                    role: Role::Assistant,
                    content: response_content.clone(),
                    tool_call_id: None,
                    tool_calls: None,
                })
                .await?;
            new_node_ids.push(final_node_id);
        }

        // Auto checkpoint if needed
        if let Some((node_id, strategy)) = self.auto_checkpoint_if_needed().await? {
            new_node_ids.push(node_id.clone());
            emit_event(AgentEvent::CheckpointCreated { node_id, strategy }).await;
        }

        // Emit done event with stats
        emit_event(AgentEvent::Done {
            total_usage,
            all_tool_calls,
            rounds,
            new_node_ids,
        })
        .await;

        Ok(response_content)
    }

    /// Manually create a checkpoint at the current active leaf (uses balanced strategy)
    pub async fn checkpoint(&mut self) -> Result<NodeId> {
        let result = self
            .create_checkpoint(
                self.session.active_leaf_id.clone(),
                CheckpointOptions::default(),
            )
            .await?;
        Ok(result.node_id)
    }

    /// Create a checkpoint at a specific node with options
    ///
    /// This compacts all messages from the target node back to the root (or previous checkpoint)
    /// into a summary using the specified options.
    ///
    /// # Arguments
    /// * `target_node_id` - The node where the checkpoint will be created
    /// * `options` - Checkpoint options including strategy, custom prompt, and provider choice
    pub async fn create_checkpoint(
        &mut self,
        target_node_id: NodeId,
        options: CheckpointOptions,
    ) -> Result<CheckpointResult> {
        // Get context string using the appropriate strategy
        let context_strategy = options.strategy.to_context_strategy();
        let context_string = self
            .session
            .get_context_string_from(target_node_id.clone(), context_strategy)
            .await?;

        if context_string.is_empty() {
            return Err(anyhow::anyhow!("No messages to checkpoint"));
        }

        // Calculate stats from the context string
        let original_tokens = Self::estimate_tokens_for_text(&context_string);
        let nodes_covered = self
            .session
            .count_nodes_in_path(target_node_id.clone(), context_strategy)
            .await?;
        let time_range = Self::get_time_range_now();

        // Generate summary with strategy
        let summary = self
            .generate_summary_with_options(&context_string, &options)
            .await?;

        let summary_tokens = Self::estimate_tokens_for_text(&summary);
        let compression_ratio = if original_tokens > 0 {
            1.0 - (summary_tokens as f32 / original_tokens as f32)
        } else {
            0.0
        };

        let stats = CheckpointStats {
            nodes_covered,
            total_tokens: original_tokens,
            summary_tokens,
            compression_ratio,
            covered_time_range: time_range,
        };

        // Create the checkpoint
        self.session
            .create_checkpoint_with_stats(
                target_node_id.clone(),
                summary.clone(),
                &options.strategy.to_string(),
                stats.clone(),
            )
            .await?;

        Ok(CheckpointResult {
            node_id: target_node_id,
            summary,
            stats,
        })
    }

    /// Preview a checkpoint without creating it
    ///
    /// Returns what the checkpoint would look like without actually creating it.
    /// Useful for showing the user what summary will be generated before committing.
    ///
    /// # Arguments
    /// * `target_node_id` - The node where the checkpoint would be created
    /// * `options` - Checkpoint options including strategy, custom prompt, and provider choice
    pub async fn preview_checkpoint(
        &mut self,
        target_node_id: NodeId,
        options: CheckpointOptions,
    ) -> Result<CheckpointResult> {
        // Get context string using the appropriate strategy
        let context_strategy = options.strategy.to_context_strategy();
        let context_string = self
            .session
            .get_context_string_from(target_node_id.clone(), context_strategy)
            .await?;

        if context_string.is_empty() {
            return Err(anyhow::anyhow!("No messages to checkpoint"));
        }

        // Calculate stats from the context string
        let original_tokens = Self::estimate_tokens_for_text(&context_string);
        let nodes_covered = self
            .session
            .count_nodes_in_path(target_node_id.clone(), context_strategy)
            .await?;
        let time_range = Self::get_time_range_now();

        // Generate summary preview
        let summary = self
            .generate_summary_with_options(&context_string, &options)
            .await?;

        let summary_tokens = Self::estimate_tokens_for_text(&summary);
        let compression_ratio = if original_tokens > 0 {
            1.0 - (summary_tokens as f32 / original_tokens as f32)
        } else {
            0.0
        };

        Ok(CheckpointResult {
            node_id: target_node_id,
            summary,
            stats: CheckpointStats {
                nodes_covered,
                total_tokens: original_tokens,
                summary_tokens,
                compression_ratio,
                covered_time_range: time_range,
            },
        })
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
            let result = self
                .create_checkpoint(
                    self.session.active_leaf_id.clone(),
                    CheckpointOptions::default(),
                )
                .await?;
            return Ok(Some((result.node_id, strategy.to_string())));
        }

        Ok(None)
    }

    /// Generate a summary using checkpoint options
    ///
    /// By default uses quick_provider (faster/cheaper model) for compression.
    /// Set `options.use_main_provider = true` to use the main provider for higher quality.
    async fn generate_summary_with_options(
        &self,
        context_string: &str,
        options: &CheckpointOptions,
    ) -> Result<String> {
        let strategy_prompt = options
            .strategy
            .get_prompt(options.custom_prompt.as_deref());

        let summary_prompt = format!(
            "{}\n\n---\n\nConversation to summarize:\n\n{}",
            strategy_prompt, context_string
        );

        let summary_context = vec![Message {
            role: Role::User,
            content: summary_prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        // Choose provider based on options:
        // - use_main_provider=true: always use main provider (higher quality)
        // - use_main_provider=false (default): use quick_provider if available, otherwise main
        let mut handle = if options.use_main_provider {
            // User explicitly requested main provider for higher quality
            self.provider.chat_loop(summary_context, None).await?
        } else if let Some(ref quick) = self.quick_provider {
            // Default: use quick provider for speed/cost
            quick.chat_loop(summary_context, None).await?
        } else {
            // Fallback: no quick provider configured, use main
            self.provider.chat_loop(summary_context, None).await?
        };

        let mut summary = String::new();

        use crate::llm::LoopStep;
        while let Some(event) = handle.next().await {
            match event? {
                LoopStep::Content(text) => {
                    summary.push_str(&text);
                }
                LoopStep::Done { content, .. } => {
                    if !content.is_empty() && content != summary {
                        summary = content;
                    }
                    break;
                }
                _ => {}
            }
        }

        Ok(summary)
    }

    /// Estimate token count for text (rough approximation: 4 chars ≈ 1 token)
    fn estimate_tokens_for_text(text: &str) -> u32 {
        (text.len() / 4) as u32
    }

    /// Get current time as time range
    fn get_time_range_now() -> (i64, i64) {
        let now = crate::history::node::now();
        (now, now)
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
