//! Agent factory for creating configured agent instances
//!
//! Provides a factory pattern for creating agents with consistent configuration,
//! tool registries, and runtime dependencies.

use anyhow::{Context, Result};
use std::sync::Arc;

use crate::agent::{Agent, AgentRuntime, SpawnSubAgentTool, SubAgentRegistry};
use crate::api::event_bus::GlobalEventBus;
use crate::history::{Session, TreeStore};
use crate::llm::{ActiveProvider, LLMProvider, ToolRegistry};

/// Factory for creating configured agent instances
///
/// Centralizes agent creation logic to ensure consistent configuration
/// across main agents and sub-agents.
pub struct AgentFactory {
    /// Provider factory for creating new LLM providers
    provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,

    /// Base tool registry (cloned for each agent)
    base_tools: ToolRegistry,

    /// Agent runtime for run tracking
    runtime: Arc<AgentRuntime>,

    /// Sub-agent registry
    registry: Arc<SubAgentRegistry>,

    /// Event bus for injection
    event_bus: Arc<GlobalEventBus>,

    /// Storage backend
    storage: Arc<dyn TreeStore>,

    /// Maximum concurrent sub-agents
    max_concurrent: usize,
}

impl AgentFactory {
    /// Create a new AgentFactory
    ///
    /// # Arguments
    /// * `provider_factory` - Factory function that creates new LLM providers
    /// * `base_tools` - Base tool registry (will be cloned for each agent)
    /// * `runtime` - Agent runtime for run tracking
    /// * `registry` - Sub-agent registry
    /// * `event_bus` - Event bus for message injection
    /// * `storage` - Storage backend for sessions
    /// * `max_concurrent` - Maximum concurrent sub-agents (default: 8)
    pub fn new(
        provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
        base_tools: ToolRegistry,
        runtime: Arc<AgentRuntime>,
        registry: Arc<SubAgentRegistry>,
        event_bus: Arc<GlobalEventBus>,
        storage: Arc<dyn TreeStore>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            provider_factory,
            base_tools,
            runtime,
            registry,
            event_bus,
            storage,
            max_concurrent,
        }
    }

    /// Create a new agent instance with spawn tool support
    ///
    /// # Arguments
    /// * `session` - Session for this agent
    /// * `session_key` - Unique session key
    /// * `include_spawn_tool` - Whether to register the spawn_subagent tool
    ///
    /// # Returns
    /// Configured agent instance ready for use
    pub fn create_agent(
        &self,
        session: Session,
        session_key: String,
        include_spawn_tool: bool,
    ) -> Result<Agent<ActiveProvider>> {
        // Create provider
        let provider = (self.provider_factory)();

        // Clone base tools
        let mut tools = self.base_tools.clone();

        // Add spawn tool if requested
        if include_spawn_tool {
            let spawn_tool = SpawnSubAgentTool::new(
                Arc::clone(&self.registry),
                Arc::clone(&self.runtime),
                self.max_concurrent,
                Arc::clone(&self.provider_factory),
                Arc::clone(&self.storage),
                session_key.clone(),
                Arc::clone(&self.event_bus),
            );
            tools = tools.register(spawn_tool);
        }

        // Create agent
        let mut agent = Agent::new(session, provider, tools);
        agent.set_runtime(Arc::clone(&self.runtime));
        agent.set_session_key(session_key);

        Ok(agent)
    }

    /// Create a sub-agent (without spawn tool to prevent nesting)
    ///
    /// This is a convenience method that always creates agents without spawn tool support.
    pub fn create_subagent(
        &self,
        session: Session,
        session_key: String,
    ) -> Result<Agent<ActiveProvider>> {
        self.create_agent(session, session_key, false)
    }

    /// Create a main agent (with spawn tool support)
    ///
    /// This is a convenience method that always creates agents with spawn tool support.
    pub fn create_main_agent(
        &self,
        session: Session,
        session_key: String,
    ) -> Result<Agent<ActiveProvider>> {
        self.create_agent(session, session_key, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{MemoryStore, SessionConfig};
    use crate::llm::OpenAIProvider;
    use tempfile::TempDir;

    fn create_test_factory() -> (AgentFactory, TempDir) {
        let temp = TempDir::new().unwrap();
        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let runtime = Arc::new(AgentRuntime::new());
        let registry = Arc::new(SubAgentRegistry::new(temp.path().join("registry.json")));
        let event_bus = Arc::new(GlobalEventBus::new());

        let provider_factory = Arc::new(|| {
            ActiveProvider::OpenAI(
                OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string())
                    .expect("Failed to create provider"),
            )
        });

        let base_tools = ToolRegistry::new().register_all_builtin();

        let factory = AgentFactory::new(
            provider_factory,
            base_tools,
            runtime,
            registry,
            event_bus,
            storage,
            8,
        );

        (factory, temp)
    }

    #[tokio::test]
    async fn test_create_agent_with_spawn_tool() {
        let (factory, _temp) = create_test_factory();

        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session = Session::new(storage, SessionConfig::default())
            .await
            .unwrap();

        let _agent = factory
            .create_agent(session, "test-session".to_string(), true)
            .unwrap();

        // Agent created successfully with spawn tool
        // (tools are private, can't directly verify, but creation succeeded)
    }

    #[tokio::test]
    async fn test_create_agent_without_spawn_tool() {
        let (factory, _temp) = create_test_factory();

        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session = Session::new(storage, SessionConfig::default())
            .await
            .unwrap();

        let _agent = factory
            .create_agent(session, "test-session".to_string(), false)
            .unwrap();

        // Agent created successfully without spawn tool
    }

    #[tokio::test]
    async fn test_create_main_agent() {
        let (factory, _temp) = create_test_factory();

        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session = Session::new(storage, SessionConfig::default())
            .await
            .unwrap();

        let _agent = factory
            .create_main_agent(session, "main-agent".to_string())
            .unwrap();

        // Main agent created successfully (with spawn tool)
    }

    #[tokio::test]
    async fn test_create_subagent() {
        let (factory, _temp) = create_test_factory();

        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session = Session::new(storage, SessionConfig::default())
            .await
            .unwrap();

        let _agent = factory
            .create_subagent(session, "sub-agent".to_string())
            .unwrap();

        // Sub-agent created successfully (without spawn tool)
    }
}
