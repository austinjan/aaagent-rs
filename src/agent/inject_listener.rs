//! Message injection listener
//!
//! Subscribes to InjectMessageEvent and starts new agent turns when
//! sub-agents complete and inject their results into the main conversation.

use anyhow::{Context, Result};

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::agent::{AgentFactory, SessionManager};
use crate::api::event_bus::{GlobalEventBus, InjectMessageEvent};

/// Start the inject listener
///
/// This spawns a background task that:
/// 1. Subscribes to InjectMessageEvent from GlobalEventBus
/// 2. Gets or creates the session for the target agent
/// 3. Creates an agent instance via AgentFactory
/// 4. Calls agent.chat() with the injected message
/// 5. Handles the response (currently just logs it)
///
/// # Arguments
/// * `event_bus` - Event bus to subscribe to
/// * `session_manager` - Session manager for getting/creating sessions
/// * `factory` - Agent factory for creating agent instances
///
/// # Returns
/// A join handle for the background task
pub fn start_inject_listener(
    event_bus: Arc<GlobalEventBus>,
    session_manager: Arc<SessionManager>,
    factory: Arc<AgentFactory>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = event_bus.subscribe_inject();

        loop {
            match rx.recv().await {
                Ok(event) => {
                    // Process inject event
                    if let Err(e) = handle_inject_event(
                        event,
                        Arc::clone(&session_manager),
                        Arc::clone(&factory),
                    )
                    .await
                    {
                        eprintln!("Error handling inject event: {}", e);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    eprintln!("Inject listener lagged, skipped {} messages", skipped);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    eprintln!("Inject event channel closed, exiting listener");
                    break;
                }
            }
        }
    })
}

/// Handle a single inject event
async fn handle_inject_event(
    event: InjectMessageEvent,
    session_manager: Arc<SessionManager>,
    factory: Arc<AgentFactory>,
) -> Result<()> {
    // Get or create session
    let session_arc = session_manager
        .get_or_create(event.session_key.clone())
        .await
        .context("Failed to get or create session")?;

    // Create agent with main agent capabilities (includes spawn tool)
    let session = {
        let guard = session_arc.read().await;
        guard.clone()
    };

    let mut agent = factory
        .create_main_agent(session, event.session_key.clone())
        .context("Failed to create agent")?;

    // Start new turn with injected message
    // Note: This is async and will run in the background
    match agent.chat(&event.message).await {
        Ok(response) => {
            println!(
                "[Inject] Agent {} processed message, response: {}",
                event.session_key,
                response.chars().take(100).collect::<String>()
            );
        }
        Err(e) => {
            eprintln!(
                "[Inject] Agent {} error processing message: {}",
                event.session_key, e
            );
        }
    }

    // Update session back to manager
    {
        let mut guard = session_arc.write().await;
        *guard = agent.session;
    }

    // Persist session to disk
    session_manager
        .persist(&event.session_key)
        .await
        .context("Failed to persist session after inject")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentRuntime, SubAgentRegistry};
    use crate::api::event_bus::{GlobalEventBus, MessageSource};
    use crate::history::{MemoryStore, SessionConfig, TreeStore};
    use crate::llm::{ActiveProvider, OpenAIProvider, ToolRegistry};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_start_inject_listener() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(GlobalEventBus::new());
        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session_manager = Arc::new(SessionManager::new(
            Arc::clone(&storage),
            SessionConfig::default(),
            Some(temp.path().to_path_buf()),
        ));

        let runtime = Arc::new(AgentRuntime::new());
        let registry = Arc::new(SubAgentRegistry::new(temp.path().join("registry.json")));

        let provider_factory = Arc::new(|| {
            ActiveProvider::OpenAI(
                OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string())
                    .expect("Failed to create provider"),
            )
        });

        let base_tools = ToolRegistry::new().register_all_builtin();

        let factory = Arc::new(AgentFactory::new(
            provider_factory,
            base_tools,
            runtime,
            registry,
            Arc::clone(&event_bus),
            storage,
            8,
        ));

        // Start listener
        let _handle = start_inject_listener(
            Arc::clone(&event_bus),
            Arc::clone(&session_manager),
            Arc::clone(&factory),
        );

        // Wait a bit for listener to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Emit test event (listener will process it in background)
        event_bus.emit_inject(
            "test-session".to_string(),
            "Test message from sub-agent".to_string(),
            MessageSource::SubAgent {
                run_id: "test-run-123".to_string(),
            },
        );

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Verify session was created
        assert_eq!(session_manager.cache_size().await, 1);
    }

    #[tokio::test]
    async fn test_handle_inject_event_creates_session() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(GlobalEventBus::new());
        let storage: Arc<dyn TreeStore> = Arc::new(MemoryStore::new());
        let session_manager = Arc::new(SessionManager::new(
            Arc::clone(&storage),
            SessionConfig::default(),
            Some(temp.path().to_path_buf()),
        ));

        let runtime = Arc::new(AgentRuntime::new());
        let registry = Arc::new(SubAgentRegistry::new(temp.path().join("registry.json")));

        let provider_factory = Arc::new(|| {
            ActiveProvider::OpenAI(
                OpenAIProvider::new("gpt-4o".to_string(), "test-key".to_string())
                    .expect("Failed to create provider"),
            )
        });

        let base_tools = ToolRegistry::new().register_all_builtin();

        let factory = Arc::new(AgentFactory::new(
            provider_factory,
            base_tools,
            runtime,
            registry,
            Arc::clone(&event_bus),
            storage,
            8,
        ));

        let event = InjectMessageEvent {
            session_key: "test-session".to_string(),
            message: "Test message".to_string(),
            source: MessageSource::SubAgent {
                run_id: "test-run-456".to_string(),
            },
            timestamp: Utc::now(),
        };

        // Handle event (will fail at chat() since we don't have real API key, but that's ok)
        let _ =
            handle_inject_event(event, Arc::clone(&session_manager), Arc::clone(&factory)).await;

        // Session should be created despite chat failure
        assert_eq!(session_manager.cache_size().await, 1);
    }
}
