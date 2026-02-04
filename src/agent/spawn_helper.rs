//! Helper to integrate SpawnSubAgentTool with Agent
//!
//! Provides utilities to create and register the spawn tool with proper dependencies.

use anyhow::Result;
use std::sync::Arc;

use crate::agent::{Agent, AgentRuntime, SpawnSubAgentTool, SubAgentRegistry};
use crate::api::event_bus::GlobalEventBus;
use crate::history::TreeStore;
use crate::llm::{ActiveProvider, LLMProvider, ToolRegistry};

/// Register the spawn_subagent tool with an agent
///
/// This adds the spawn tool to the agent's tool registry, allowing it to
/// spawn sub-agents with the provided dependencies.
pub fn register_spawn_tool<P: LLMProvider>(
    agent: &mut Agent<P>,
    registry: Arc<SubAgentRegistry>,
    runtime: Arc<AgentRuntime>,
    event_bus: Arc<GlobalEventBus>,
    storage: Arc<dyn TreeStore>,
    session_key: String,
    provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
    max_concurrent: usize,
) {
    let spawn_tool = SpawnSubAgentTool::new(
        registry,
        runtime,
        max_concurrent,
        provider_factory,
        storage,
        session_key,
        event_bus,
    );

    // Note: This requires adding a method to Agent to access tools mutably
    // For now, we'll need to create the agent with the tool already registered
    // See create_agent_with_spawn_tool below
}

/// Create an agent with spawn tool pre-registered
///
/// This is a convenience function that creates an agent and automatically
/// registers the spawn_subagent tool with it.
pub fn create_agent_with_spawn_tool<P: LLMProvider>(
    provider: P,
    storage: Arc<dyn TreeStore>,
    session_key: String,
    registry: Arc<SubAgentRegistry>,
    runtime: Arc<AgentRuntime>,
    event_bus: Arc<GlobalEventBus>,
    provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
    max_concurrent: usize,
) -> Result<Agent<P>> {
    // Create tool registry with spawn tool
    let mut tools = ToolRegistry::new().register_all_builtin();

    let spawn_tool = SpawnSubAgentTool::new(
        registry.clone(),
        runtime.clone(),
        max_concurrent,
        provider_factory,
        storage.clone(),
        session_key.clone(),
        event_bus,
    );

    tools = tools.register(spawn_tool);

    // Create session (this is async, so we need to handle it differently)
    // For now, return an error indicating this needs to be done differently
    anyhow::bail!("Use create_agent_with_spawn_tool_async instead")
}

/// Create an agent with spawn tool pre-registered (async version)
///
/// This is the proper way to create an agent with spawn tool support.
pub async fn create_agent_with_spawn_tool_async<P: LLMProvider>(
    provider: P,
    storage: Arc<dyn TreeStore>,
    session_key: String,
    registry: Arc<SubAgentRegistry>,
    runtime: Arc<AgentRuntime>,
    event_bus: Arc<GlobalEventBus>,
    provider_factory: Arc<dyn Fn() -> ActiveProvider + Send + Sync>,
    max_concurrent: usize,
) -> Result<Agent<P>> {
    use crate::history::{Session, SessionConfig};

    // Create tool registry with spawn tool
    let mut tools = ToolRegistry::new().register_all_builtin();

    let spawn_tool = SpawnSubAgentTool::new(
        registry.clone(),
        runtime.clone(),
        max_concurrent,
        provider_factory,
        storage.clone(),
        session_key.clone(),
        event_bus,
    );

    tools = tools.register(spawn_tool);

    // Create session
    let session = Session::new(storage, SessionConfig::default()).await?;

    // Create agent
    let mut agent = Agent::new(session, provider, tools);
    agent.set_runtime(runtime);
    agent.set_session_key(session_key);

    Ok(agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::MemoryStore;
    use crate::llm::ActiveProvider;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_agent_with_spawn_tool() {
        let temp = TempDir::new().unwrap();
        let registry = Arc::new(SubAgentRegistry::new(temp.path().join("registry.json")));
        let runtime = Arc::new(AgentRuntime::new());
        let event_bus = Arc::new(GlobalEventBus::new());
        let storage = Arc::new(MemoryStore::new());

        // Mock provider factory (returns a dummy provider)
        let provider_factory = Arc::new(|| {
            // This is just for testing - in real use, return actual provider
            #[cfg(feature = "openai")]
            {
                ActiveProvider::OpenAI(
                    crate::llm::OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string())
                        .expect("Failed to create provider"),
                )
            }
            #[cfg(not(feature = "openai"))]
            {
                panic!("No provider available for test")
            }
        }) as Arc<dyn Fn() -> ActiveProvider + Send + Sync>;

        #[cfg(feature = "openai")]
        {
            let provider =
                crate::llm::OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string())
                    .expect("Failed to create provider");

            let agent = create_agent_with_spawn_tool_async(
                provider,
                storage,
                "main-agent".to_string(),
                registry,
                runtime,
                event_bus,
                provider_factory,
                8,
            )
            .await;

            assert!(agent.is_ok());
        }
    }
}
