use axum::{
    extract::{Path, State},
    http::{
        header::{HeaderName, AUTHORIZATION},
        StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;

#[cfg(feature = "dev-server")]
use tower_http::cors::CorsLayer;

use crate::config::{ConfigResolver, SessionConfig};
use crate::history::{SessionConfig as HistorySessionConfig, TreeStore};
use crate::llm::LLMProvider;

mod provider_factory;
mod stream_manager;

use stream_manager::StreamManager;

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub store: Arc<crate::history::JSONLStore>,
    pub stream_manager: Arc<StreamManager>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        // Ensure data directories exist
        std::fs::create_dir_all("data")?;
        std::fs::create_dir_all("data/temp")?;

        // Load configuration
        let config_resolver = Arc::new(ConfigResolver::new()?);

        // Initialize JSONL store with lazy loading
        crate::logger::log(format!("[Startup] Initializing JSONL store (sync_every: {})", 
            std::env::var("AAAGENT_JSONL_SYNC_EVERY").unwrap_or_else(|_| "1".to_string())));
        let sync_every_writes = std::env::var("AAAGENT_JSONL_SYNC_EVERY")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1);
        let store = Arc::new(
            crate::history::JSONLStore::new_with_sync_every(
                "data".into(),
                sync_every_writes,
            )
            .await?,
        );

        // Run startup cleanup in background
        let maintenance_config = config_resolver
            .config_manager()
            .maintenance_config()
            .clone();
        tokio::spawn(async move {
            crate::logger::log("[Startup] Running initial cleanup...".to_string());
            let results = crate::maintenance::run_cleanup_tasks(&maintenance_config).await;
            for (task, result) in results {
                match result {
                    Ok(count) if count > 0 => crate::logger::log(format!(
                        "[Startup] Cleanup '{}': {} items removed",
                        task, count
                    )),
                    Ok(_) => {} // Skip logging if 0 items removed
                    Err(e) => {
                        crate::logger::log(format!("[Startup] Cleanup '{}' failed: {}", task, e))
                    }
                }
            }
        });

        Ok(Self {
            config_resolver,
            store,
            stream_manager: Arc::new(StreamManager::new()),
        })
    }
}

pub async fn create_router() -> Router {
    let state = AppState::new()
        .await
        .expect("Failed to initialize app state");

    // Start background maintenance worker
    let maintenance_config = state
        .config_resolver
        .config_manager()
        .maintenance_config()
        .clone();
    crate::maintenance::start_maintenance_worker(maintenance_config);

    // Define sensitive headers that should never be logged
    let sensitive_headers = vec![
        AUTHORIZATION,
        HeaderName::from_static("x-api-key"),
        HeaderName::from_static("anthropic-version"),
        HeaderName::from_static("x-goog-api-key"),
    ];

    let router = Router::new()
        // API routes (under /api prefix)
        .nest("/api", api_routes())
        // Static files (fallback for everything else)
        .fallback(crate::web::static_handler)
        .with_state(state)
        // Redact sensitive headers in logs/traces
        .layer(SetSensitiveRequestHeadersLayer::new(sensitive_headers));

    // Add CORS middleware only in dev mode
    #[cfg(feature = "dev-server")]
    let router = router.layer(CorsLayer::permissive());

    router
}

fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions", post(sessions::create_session))
        .route("/sessions/:session_id", get(sessions::get_session))
        .route("/sessions/:session_id/chat", post(sessions::chat))
        .route("/sessions/:session_id/stream/:stream_id", get(sse::stream))
        .route("/sessions/:session_id/path", get(sessions::get_path))
        .route(
            "/sessions/:session_id/path/metadata",
            get(sessions::get_metadata),
        )
        .route(
            "/sessions/:session_id/checkpoints",
            get(sessions::get_checkpoints),
        )
        .route(
            "/sessions/:session_id/system-prompt",
            get(sessions::get_system_prompt),
        )
        .route("/sessions/:session_id/config", get(sessions::get_config))
        .route(
            "/sessions/:session_id/config",
            axum::routing::patch(sessions::update_config),
        )
}

// Health check endpoint
async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": "aaagent-rs chat UI backend is running",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// Error type for API responses
enum ApiError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

// Request/Response types
#[derive(Debug, Serialize, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    stream_id: String,
}

#[derive(Debug, Serialize)]
struct ConfigResponse {
    session_config: SessionConfig,
}

// Placeholder handlers
mod sessions {
    use super::*;

    pub async fn list_sessions(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
        let sessions = state
            .store
            .list_sessions()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Convert to session summaries
        let summaries: Vec<_> = sessions
            .iter()
            .map(|s| {
                json!({
                    "session_id": s.session_id,
                    "name": s.name,
                    "created_at": s.created_at,
                    "updated_at": s.updated_at,
                    "message_count": s.stats.total_nodes,
                })
            })
            .collect();

        Ok(Json(json!({
            "sessions": summaries,
            "total": sessions.len()
        })))
    }

    pub async fn create_session(
        State(state): State<AppState>,
        Json(req): Json<Value>,
    ) -> Result<Json<Value>, ApiError> {
        use crate::history::Session;

        let name = req
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Session");
        let session_config = state
            .config_resolver
            .default_session_config()
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Create session with persistent store
        let store = state.store.clone();
        let session_config_for_history = HistorySessionConfig {
            system_prompt: session_config.session.system_prompt.clone().into(),
            max_context_tokens: session_config.session.max_context_tokens,
            ..Default::default()
        };

        let mut session = Session::new(store.clone(), session_config_for_history)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Set session name and metadata
        session.name = Some(name.to_string());
        session.metadata = Some(json!({
            "session_config": session_config,
        }));

        // Save updated session with metadata
        store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(json!({
            "session_id": session.session_id,
            "name": session.name,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "session_config": session_config
        })))
    }

    pub async fn get_session(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Extract preset from metadata if available
        Ok(Json(json!({
            "session_id": session.session_id,
            "name": session.name,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "message_count": session.stats.total_nodes,
            "root_node_id": session.root_node_id,
            "active_leaf_id": session.active_leaf_id
        })))
    }

    pub async fn chat(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<ChatRequest>,
    ) -> Result<Json<ChatResponse>, ApiError> {
        crate::logger::log(format!("Chat request received for session: {}", session_id));

        // Validate request
        if req.message.trim().is_empty() {
            return Err(ApiError::BadRequest("message cannot be empty".to_string()));
        }

        // Load session from storage
        let stored_session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        let session_config = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("session_config"))
            .and_then(|c| serde_json::from_value::<SessionConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing session_config".to_string()))?;

        // Create stream
        let (stream_id, tx) = state.stream_manager.create_stream().await;
        crate::logger::log(format!(
            "Created stream: {} for session: {}",
            stream_id, session_id
        ));

        // Clone values for the background task
        let message = req.message.clone();
        let config_manager = state.config_resolver.clone();
        let stream_id_clone = stream_id.clone();
        let session_config_for_task = session_config.clone();
        let store_for_task = state.store.clone();

        // Spawn background task to run Agent
        tokio::spawn(async move {
            crate::logger::log(format!(
                "Starting agent chat for stream: {}",
                stream_id_clone
            ));

            let result = run_agent_chat(
                stored_session,
                message,
                session_config_for_task,
                config_manager,
                store_for_task,
                tx.clone(),
            )
            .await;

            // If agent failed, send error to frontend
            match result {
                Ok(_) => {
                    crate::logger::log(format!(
                        "Agent chat completed successfully for stream: {}",
                        stream_id_clone
                    ));
                }
                Err(e) => {
                    crate::logger::log(format!(
                        "ERROR: Agent chat failed for stream {}: {}",
                        stream_id_clone, e
                    ));
                    crate::logger::log(format!("ERROR: Error details: {:?}", e));

                    // Send error message to frontend
                    let error_msg = format!("❌ Agent Error: {}\n\nDetails: {}", e, e);
                    crate::logger::log(format!("Sending error message to stream: {}", error_msg));

                    match tx
                        .send(crate::agent::AgentEvent::Content(error_msg.clone()))
                        .await
                    {
                        Ok(_) => {
                            crate::logger::log("Error message sent successfully".to_string())
                        }
                        Err(send_err) => crate::logger::log(format!(
                            "ERROR: Failed to send error message: {}",
                            send_err
                        )),
                    }

                    // Send done event
                    let _ = tx
                        .send(crate::agent::AgentEvent::Done {
                            total_usage: crate::llm::TokenUsage {
                                input_tokens: 0,
                                output_tokens: 0,
                                cached_tokens: 0,
                            },
                            all_tool_calls: vec![],
                            rounds: 0,
                        })
                        .await;

                    // Drop sender to close channel cleanly
                    drop(tx);
                    crate::logger::log("Sender dropped, channel closed cleanly".to_string());

                    // Wait for SSE to read messages
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }
            }

            // TODO: Save updated session back to storage
            // For now, sessions are not persisted after chat
        });

        Ok(Json(ChatResponse { stream_id }))
    }

    pub async fn get_path(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        // Load session
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Get path nodes from active leaf to root
        let leaf_id = session.active_leaf_id.clone();
        let nodes = state
            .store
            .get_path_to_root_internal(leaf_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Reverse to get chronological order (root -> leaf)
        let mut nodes = nodes;
        nodes.reverse();

        Ok(Json(json!({ "nodes": nodes })))
    }

    pub async fn get_metadata(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        Ok(Json(json!({
            "total_nodes": session.stats.total_nodes,
            "active_branches": session.stats.active_branches,
            "total_checkpoints": session.stats.total_checkpoints,
            "total_tokens_processed": session.stats.total_tokens_processed,
        })))
    }

    pub async fn get_checkpoints(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        Ok(Json(json!({ "checkpoints": session.checkpoints })))
    }

    pub async fn get_system_prompt(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        Ok(Json(json!({
            "prompt": session.config.system_prompt.unwrap_or_default()
        })))
    }

    pub async fn get_config(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<ConfigResponse>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        let session_config = session
            .metadata
            .as_ref()
            .and_then(|m| m.get("session_config"))
            .and_then(|c| serde_json::from_value::<SessionConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing session_config".to_string()))?;

        Ok(Json(ConfigResponse { session_config }))
    }

    pub async fn update_config(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(config): Json<SessionConfig>,
    ) -> Result<Json<SessionConfig>, ApiError> {
        // Load existing session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        state
            .config_resolver
            .validate(&config)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        // Update session metadata
        if let Some(ref mut metadata) = session.metadata {
            metadata["session_config"] =
                serde_json::to_value(&config).map_err(|e| ApiError::Internal(e.to_string()))?;
        } else {
            session.metadata = Some(json!({
                "session_config": config,
            }));
        }
        session.config.system_prompt = Some(config.session.system_prompt.clone());
        session.config.max_context_tokens = config.session.max_context_tokens;
        session.updated_at = crate::history::node::now();

        // Save updated session (automatically persisted by store)
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(config))
    }
}

/// Run agent chat in background task
async fn run_agent_chat(
    session: crate::history::Session,
    message: String,
    session_config: SessionConfig,
    config_manager: Arc<ConfigResolver>,
    store: Arc<crate::history::JSONLStore>,
    tx: tokio::sync::mpsc::Sender<crate::agent::AgentEvent>,
) -> anyhow::Result<()> {
    use crate::agent::{Agent, AgentConfig};
    use crate::llm::{LoopDetectorConfig, ToolRegistry};

    // Attach the shared store to the session
    let mut session = session;
    session.set_store(store.clone());

    // Create provider from resolved config
    let provider =
        provider_factory::create_provider(&session_config, config_manager.config_manager())?;

    provider.update_config(Box::new({
        let provider_config = session_config.provider.clone();
        move |cfg| {
            cfg.temperature = provider_config.temperature;
            cfg.max_tokens = provider_config.max_tokens;
            cfg.top_p = provider_config.top_p;
        }
    }));

    // Create agent with consolidated logging
    let registry = ToolRegistry::new().register_all_builtin();
    let mut agent = Agent::new(session, provider, registry);
    agent.set_config(AgentConfig {
        max_rounds: session_config.agent.max_rounds as usize,
        loop_detection: Some(LoopDetectorConfig::default()),
    });

    crate::logger::log(format!(
        "run_agent_chat: Agent initialized (model: {}, max_rounds: {})",
        session_config.provider.model, session_config.agent.max_rounds
    ));

    crate::logger::log(format!(
        "run_agent_chat: Starting chat with message: {}",
        message
    ));
    // Run chat with callback to stream events
    let result = agent
        .chat_with_callback(&message, |event| {
            let tx = tx.clone();
            async move {
                // Debug logging disabled for performance
                // crate::logger::log(format!("DEBUG: Sending event: {:?}", event));
                // Send event through channel (ignore if channel is closed)
                let _ = tx.send(event).await;
            }
        })
        .await;

    match result {
        Ok(_) => {
            crate::logger::log("run_agent_chat: Chat completed successfully".to_string());
            Ok(())
        }
        Err(e) => {
            // Just return the error - it will be handled by the spawned task
            Err(e)
        }
    }
}

mod sse {
    use super::{ApiError, AppState};
    use axum::{
        extract::{Path, State},
        response::sse::{Event, Sse},
    };
    use futures::stream::Stream;
    use std::{convert::Infallible, time::Duration};
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;

    pub async fn stream(
        Path((_session_id, stream_id)): Path<(String, String)>,
        State(state): State<AppState>,
    ) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
        crate::logger::log(format!("SSE stream requested: {}", stream_id));

        // Take the stream from the manager
        let receiver = state
            .stream_manager
            .take_stream(&stream_id)
            .await
            .ok_or_else(|| {
                crate::logger::log(format!("ERROR: Stream {} not found", stream_id));
                ApiError::NotFound(format!("Stream {} not found", stream_id))
            })?;

        crate::logger::log(format!("SSE stream connection established: {}", stream_id));

        // Convert mpsc::Receiver to Stream
        let event_stream = ReceiverStream::new(receiver).map(|agent_event| {
            // Convert AgentEvent to SSE Event
            let (event_type, data) = match agent_event {
                crate::agent::AgentEvent::Content(content) => {
                    ("content", serde_json::json!({ "content": content }))
                }
                crate::agent::AgentEvent::Thinking(text) => {
                    ("thinking", serde_json::json!({ "text": text }))
                }
                crate::agent::AgentEvent::ToolCallsRequested { tool_calls } => (
                    "tool_calls",
                    serde_json::json!({ "tool_calls": tool_calls }),
                ),
                crate::agent::AgentEvent::ToolResult {
                    tool_call_id,
                    tool_name,
                    result,
                    is_error,
                } => (
                    "tool_result",
                    serde_json::json!({
                        "tool_call_id": tool_call_id,
                        "tool_name": tool_name,
                        "result": result,
                        "is_error": is_error
                    }),
                ),
                crate::agent::AgentEvent::LoopDetected { detection } => (
                    "loop_detected",
                    serde_json::json!({ "detection": format!("{:?}", detection) }),
                ),
                crate::agent::AgentEvent::CheckpointCreated { node_id, strategy } => (
                    "checkpoint",
                    serde_json::json!({ "node_id": node_id, "strategy": strategy }),
                ),
                crate::agent::AgentEvent::Done {
                    total_usage,
                    all_tool_calls,
                    rounds,
                } => (
                    "done",
                    serde_json::json!({
                        "total_usage": total_usage,
                        "all_tool_calls": all_tool_calls,
                        "rounds": rounds
                    }),
                ),
            };

            // Create SSE event
            Ok(Event::default().event(event_type).data(data.to_string()))
        });

        // Create SSE response with keepalive
        Ok(Sse::new(event_stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        ))
    }
}
