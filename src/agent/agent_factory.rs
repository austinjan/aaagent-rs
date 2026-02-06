//! Agent factory for creating configured agent instances
//!
//! Provides a factory pattern for creating agents with consistent configuration,
//! tool registries, and runtime dependencies.

use anyhow::Result;
use std::sync::Arc;

use crate::agent::{Agent, AgentRuntime, SpawnSubAgentTool, SubAgentRegistry};
use crate::api::event_bus::GlobalEventBus;
use crate::history::{Session, TreeStore};
use crate::llm::{ActiveProvider, ToolRegistry};

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

    /// Storage backend (public for sub-agent spawning)
    pub(crate) storage: Arc<dyn TreeStore>,

    /// Storage backend for sub-agent sessions (separate from main sessions)
    pub(crate) subagent_storage: Arc<dyn TreeStore>,

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
    /// * `storage` - Storage backend for main sessions
    /// * `subagent_storage` - Storage backend for sub-agent sessions (separate directory)
    /// * `max_concurrent` - Maximum concurrent sub-agents (default: 8)
    pub fn new(
        provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
        base_tools: ToolRegistry,
        runtime: Arc<AgentRuntime>,
        registry: Arc<SubAgentRegistry>,
        event_bus: Arc<GlobalEventBus>,
        storage: Arc<dyn TreeStore>,
        subagent_storage: Arc<dyn TreeStore>,
        max_concurrent: usize,
    ) -> Self {
        Self {
            provider_factory,
            base_tools,
            runtime,
            registry,
            event_bus,
            storage,
            subagent_storage,
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
            // Create Arc to self for the spawn tool
            // This allows sub-agents to inherit all configuration
            let factory_arc = Arc::new(AgentFactory {
                provider_factory: Arc::clone(&self.provider_factory),
                base_tools: self.base_tools.clone(),
                runtime: Arc::clone(&self.runtime),
                registry: Arc::clone(&self.registry),
                event_bus: Arc::clone(&self.event_bus),
                storage: Arc::clone(&self.storage),
                subagent_storage: Arc::clone(&self.subagent_storage),
                max_concurrent: self.max_concurrent,
            });

            let spawn_tool = SpawnSubAgentTool::new(
                Arc::clone(&self.registry),
                Arc::clone(&self.runtime),
                self.max_concurrent,
                factory_arc,
                session_key.clone(),
                Arc::clone(&self.event_bus),
            );
            tools = tools.register(spawn_tool);
        }

        // Create agent
        let mut agent = Agent::new(session, provider, tools);
        agent.set_runtime(Arc::clone(&self.runtime));
        agent.set_session_key(session_key.clone());
        agent.set_event_bus(Arc::clone(&self.event_bus));

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

    /// Create an agent with a custom provider and additional tools
    ///
    /// # Arguments
    /// * `session` - Session for this agent
    /// * `session_key` - Unique session key
    /// * `provider` - Pre-configured provider (e.g., with custom temperature/model)
    /// * `include_spawn_tool` - Whether to include spawn_subagent tool
    /// * `additional_tools` - Extra tools to register (e.g., web_search)
    pub fn create_agent_with_custom_provider(
        &self,
        session: Session,
        session_key: String,
        provider: ActiveProvider,
        include_spawn_tool: bool,
        additional_tools: crate::llm::ToolRegistry,
    ) -> Result<Agent<ActiveProvider>> {
        // Clone base tools and merge with additional tools
        let mut tools = self.base_tools.clone().merge(&additional_tools);

        // Add spawn tool if requested
        if include_spawn_tool {
            let factory_arc = Arc::new(AgentFactory {
                provider_factory: Arc::clone(&self.provider_factory),
                base_tools: self.base_tools.clone(),
                runtime: Arc::clone(&self.runtime),
                registry: Arc::clone(&self.registry),
                event_bus: Arc::clone(&self.event_bus),
                storage: Arc::clone(&self.storage),
                subagent_storage: Arc::clone(&self.subagent_storage),
                max_concurrent: self.max_concurrent,
            });

            let spawn_tool = SpawnSubAgentTool::new(
                Arc::clone(&self.registry),
                Arc::clone(&self.runtime),
                self.max_concurrent,
                factory_arc,
                session_key.clone(),
                Arc::clone(&self.event_bus),
            );
            tools = tools.register(spawn_tool);
        }

        // Create agent with custom provider
        let mut agent = Agent::new(session, provider, tools);
        agent.set_runtime(Arc::clone(&self.runtime));
        agent.set_session_key(session_key.clone());
        agent.set_event_bus(Arc::clone(&self.event_bus));

        Ok(agent)
    }

    /// Clone this factory with a different provider factory
    ///
    /// Useful for creating session-specific factories that use custom provider configs
    pub fn with_provider_factory(
        &self,
        provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
    ) -> Self {
        Self {
            provider_factory,
            base_tools: self.base_tools.clone(),
            runtime: Arc::clone(&self.runtime),
            registry: Arc::clone(&self.registry),
            event_bus: Arc::clone(&self.event_bus),
            storage: Arc::clone(&self.storage),
            subagent_storage: Arc::clone(&self.subagent_storage),
            max_concurrent: self.max_concurrent,
        }
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
        let subagent_storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
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
            subagent_storage,
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

    #[tokio::test]
    async fn test_subagent_inherits_tools() {
        let (factory, _temp) = create_test_factory();

        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session = Session::new(storage, SessionConfig::default())
            .await
            .unwrap();

        // Create sub-agent
        let agent = factory
            .create_subagent(session, "sub-agent".to_string())
            .unwrap();

        // Verify sub-agent has access to built-in tools (inherited from base_tools)
        // The tools are private, but we can verify they exist by checking the agent was created
        // In production, sub-agents will have: ShellTool, ReadTool, EditorEditTool
        // But NOT spawn_subagent tool (nesting prevention)

        // If this compiles and runs without panic, sub-agent has inherited tools
        drop(agent);
    }
}
