//! Spawn SubAgent Tool - Spawns background sub-agents for parallel execution
//!
//! This tool allows the main agent to delegate tasks to background sub-agents
//! that execute independently and report results back via message injection.

use anyhow::Result;
use futures::future::BoxFuture;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::agent::{
    run_announce_flow, AgentFactory, AgentRuntime, CleanupStrategy, SubAgentRegistry, SubAgentRun,
};
use crate::api::event_bus::GlobalEventBus;
use crate::history::Session;
use crate::llm::ToolCall;
use crate::tools::ToolProvider;

/// Maximum concurrent sub-agents (default)
pub const DEFAULT_MAX_CONCURRENT: usize = 8;

/// Default timeout for sub-agents (seconds)
pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Tool for spawning sub-agents
pub struct SpawnSubAgentTool {
    /// Registry for tracking sub-agent runs
    registry: Arc<SubAgentRegistry>,

    /// Runtime for the parent agent
    runtime: Arc<AgentRuntime>,

    /// Semaphore for concurrency control (lane system)
    lanes: Arc<Semaphore>,

    /// Agent factory for creating sub-agents with inherited configuration
    agent_factory: Arc<AgentFactory>,

    /// Parent session key (the agent using this tool)
    parent_session_key: String,

    /// Global event bus for message injection
    event_bus: Arc<GlobalEventBus>,
}

impl SpawnSubAgentTool {
    /// Create a new spawn tool
    pub fn new(
        registry: Arc<SubAgentRegistry>,
        runtime: Arc<AgentRuntime>,
        max_concurrent: usize,
        agent_factory: Arc<AgentFactory>,
        parent_session_key: String,
        event_bus: Arc<GlobalEventBus>,
    ) -> Self {
        Self {
            registry,
            runtime,
            lanes: Arc::new(Semaphore::new(max_concurrent)),
            agent_factory,
            parent_session_key,
            event_bus,
        }
    }

    /// Check if current agent is a sub-agent (prevents nesting)
    fn is_sub_agent(session_key: Option<&str>) -> bool {
        session_key
            .map(|key| key.starts_with("subagent-"))
            .unwrap_or(false)
    }

    /// Spawn a sub-agent in the background
    async fn spawn_background(
        task: String,
        task_label: String,
        parent_session_key: String,
        cleanup: CleanupStrategy,
        timeout_secs: u64,
        registry: Arc<SubAgentRegistry>,
        runtime: Arc<AgentRuntime>,
        lanes: Arc<Semaphore>,
        agent_factory: Arc<AgentFactory>,
        event_bus: Arc<GlobalEventBus>,
    ) -> Result<String> {
        // Generate unique IDs
        let run_id = format!("subagent-{}", ulid::Ulid::new().to_string().to_lowercase());
        let child_session_key = format!(
            "subagent-session-{}",
            ulid::Ulid::new().to_string().to_lowercase()
        );

        // Register run in registry
        let run = SubAgentRun::new(
            run_id.clone(),
            child_session_key.clone(),
            parent_session_key.clone(),
            task_label.clone(),
            cleanup,
        );

        registry.register(run.clone())?;

        // Spawn background task
        let registry_clone = registry.clone();
        let runtime_clone = runtime.clone();
        let agent_factory_clone = agent_factory.clone();
        let run_id_clone = run_id.clone();
        let task_label_clone = task_label.clone();
        let parent_session_key_clone = parent_session_key.clone();
        let event_bus_clone = event_bus.clone();

        tokio::spawn(async move {
            // Acquire lane permit (blocks if all lanes busy)
            let _permit = lanes.acquire().await.unwrap();

            log::info!(
                "Sub-agent {} started (task: {})",
                run_id_clone,
                task_label_clone
            );

            // Mark as started
            let mut run = registry_clone.get_run(&run_id_clone).unwrap();
            run.mark_started();
            let _ = registry_clone.update_run(run.clone());

            // Emit sub-agent spawned event to SSE
            event_bus_clone.emit(
                parent_session_key_clone.clone(),
                run_id_clone.clone(),
                0, // run_seq
                crate::agent::AgentEvent::SubAgentSpawned {
                    run_id: run_id_clone.clone(),
                    task_label: task_label_clone.clone(),
                },
            );

            // Clone storage before agent_factory is moved
            let storage_for_announce = agent_factory_clone.storage.clone();

            // Execute sub-agent
            let outcome = Self::execute_subagent(
                task,
                child_session_key.clone(),
                timeout_secs,
                agent_factory_clone,
                runtime_clone.clone(),
            )
            .await;

            // Mark as completed
            let mut run = registry_clone.get_run(&run_id_clone).unwrap();
            run.mark_completed(outcome.clone());
            let _ = registry_clone.update_run(run.clone());

            log::info!("Sub-agent {} completed", run_id_clone);

            // Emit sub-agent completed event to SSE
            let (success, error) = match &outcome {
                crate::agent::SubAgentOutcome::Success { .. } => (true, None),
                crate::agent::SubAgentOutcome::Error { error } => (false, Some(error.clone())),
                crate::agent::SubAgentOutcome::Timeout { .. } => {
                    (false, Some("Timeout".to_string()))
                }
            };

            event_bus_clone.emit(
                parent_session_key_clone.clone(),
                run_id_clone.clone(),
                1, // run_seq
                crate::agent::AgentEvent::SubAgentCompleted {
                    run_id: run_id_clone.clone(),
                    success,
                    error,
                },
            );

            // Trigger announce flow
            if let Some(run) = registry_clone.get_run(&run_id_clone) {
                if let Err(e) = run_announce_flow(
                    &run,
                    registry_clone.clone(),
                    runtime_clone.clone(),
                    event_bus,
                    storage_for_announce,
                )
                .await
                {
                    log::error!("Failed to announce sub-agent completion: {}", e);
                }
            }

            // Cleanup if requested
            if run.cleanup == CleanupStrategy::DeleteImmediately {
                let _ = registry_clone.remove_run(&run_id_clone);
                log::info!("Sub-agent {} session cleaned up", run_id_clone);
            }
        });

        Ok(run_id)
    }

    /// Execute the sub-agent
    async fn execute_subagent(
        task: String,
        session_key: String,
        timeout_secs: u64,
        agent_factory: Arc<AgentFactory>,
        _runtime: Arc<AgentRuntime>,
    ) -> crate::agent::SubAgentOutcome {
        use crate::agent::SubAgentOutcome;

        let start_time = std::time::Instant::now();

        // Create new session for sub-agent (use separate sub-agent storage)
        let config = crate::history::SessionConfig::default();
        let session = match Session::new(agent_factory.subagent_storage.clone(), config).await {
            Ok(s) => s,
            Err(e) => {
                return SubAgentOutcome::Error {
                    error: format!("Failed to create session: {}", e),
                };
            }
        };

        // Create sub-agent using factory (inherits all tools and config from main agent)
        let mut agent = match agent_factory.create_subagent(session, session_key.clone()) {
            Ok(a) => a,
            Err(e) => {
                return SubAgentOutcome::Error {
                    error: format!("Failed to create sub-agent: {}", e),
                };
            }
        };

        // Execute with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            agent.chat(&task),
        )
        .await;

        let elapsed = start_time.elapsed();

        match result {
            Ok(Ok(output)) => {
                // Success - get token usage from session if possible
                let tokens_used = 0; // TODO: Track tokens properly

                SubAgentOutcome::Success {
                    output,
                    tokens_used,
                    runtime_ms: elapsed.as_millis() as i64,
                }
            }
            Ok(Err(e)) => SubAgentOutcome::Error {
                error: e.to_string(),
            },
            Err(_) => SubAgentOutcome::Timeout { timeout_secs },
        }
    }
}

#[async_trait::async_trait]
impl ToolProvider for SpawnSubAgentTool {
    fn name(&self) -> &str {
        "spawn_subagent"
    }

    fn brief(&self) -> &str {
        "Spawn a background sub-agent to execute a task independently"
    }

    fn full_description(&self) -> String {
        "Spawn a background sub-agent to execute a task independently. The sub-agent runs in parallel and will report results back when complete. Use this for long-running tasks, parallel work, or delegating specialized analysis.\n\nNOTE: Sub-agents cannot spawn other sub-agents (nesting not allowed).".to_string()
    }

    fn parameters(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task for the sub-agent to execute. Be specific and include all necessary context."
                },
                "task_label": {
                    "type": "string",
                    "description": "A short human-readable label for this task (e.g., 'Search codebase for TODOs')"
                },
                "cleanup": {
                    "type": "string",
                    "enum": ["delete_immediately", "keep"],
                    "description": "Whether to delete the sub-agent session after completion or keep it for debugging. Default: keep",
                    "default": "keep"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum execution time in seconds. Default: 300 (5 minutes)",
                    "default": 300
                }
            },
            "required": ["task", "task_label"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move { self.execute_impl(call).await.map_err(|e| e.to_string()) })
    }
}

impl SpawnSubAgentTool {
    async fn execute_impl(&self, call: &ToolCall) -> Result<String> {
        let arguments = &call.arguments;
        // Parse arguments
        let task = arguments
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: task"))?
            .to_string();

        let task_label = arguments
            .get("task_label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required field: task_label"))?
            .to_string();

        let cleanup_str = arguments
            .get("cleanup")
            .and_then(|v| v.as_str())
            .unwrap_or("keep");

        let cleanup = match cleanup_str {
            "delete_immediately" => CleanupStrategy::DeleteImmediately,
            "keep" | _ => CleanupStrategy::Keep,
        };

        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        // Prevent nesting: sub-agents cannot spawn sub-agents
        if Self::is_sub_agent(Some(&self.parent_session_key)) {
            anyhow::bail!("Sub-agents cannot spawn other sub-agents (nesting not allowed)");
        }

        // Spawn background sub-agent
        let run_id = Self::spawn_background(
            task,
            task_label.clone(),
            self.parent_session_key.clone(),
            cleanup,
            timeout_secs,
            self.registry.clone(),
            self.runtime.clone(),
            self.lanes.clone(),
            self.agent_factory.clone(),
            self.event_bus.clone(),
        )
        .await?;

        Ok(format!(
            "✅ Sub-agent spawned successfully\n\n\
            Run ID: {}\n\
            Task: {}\n\
            Status: Running in background\n\n\
            The sub-agent will report results when complete.",
            run_id, task_label
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sub_agent() {
        assert!(SpawnSubAgentTool::is_sub_agent(Some("subagent-xyz")));
        assert!(SpawnSubAgentTool::is_sub_agent(Some(
            "subagent-session-abc"
        )));
        assert!(!SpawnSubAgentTool::is_sub_agent(Some("main-agent")));
        assert!(!SpawnSubAgentTool::is_sub_agent(Some("agent-123")));
        assert!(!SpawnSubAgentTool::is_sub_agent(None));
    }
}
