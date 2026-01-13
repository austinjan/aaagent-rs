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

use aaagent::config::{ChatConfig, ChatIntent, ConfigResolver, ResolvedConfig};
use aaagent::history::TreeStore;
use aaagent::storage::file_store::FileSessionStore;
use aaagent::storage::SessionStore;

mod provider_factory;
mod stream_manager;

use stream_manager::StreamManager;

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub session_store: Arc<dyn SessionStore>,
    pub stream_manager: Arc<StreamManager>,
    pub tree_store: Arc<aaagent::history::MemoryStore>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        // Ensure data directories exist
        std::fs::create_dir_all("data/sessions")?;
        std::fs::create_dir_all("data/temp")?;

        // Load configuration
        let config_resolver = Arc::new(ConfigResolver::new()?);

        // Run startup cleanup in background
        let maintenance_config = config_resolver
            .config_manager()
            .maintenance_config()
            .clone();
        tokio::spawn(async move {
            aaagent::logger::log("[Startup] Running initial cleanup...".to_string());
            let results = aaagent::maintenance::run_cleanup_tasks(&maintenance_config).await;
            for (task, result) in results {
                match result {
                    Ok(count) => aaagent::logger::log(format!(
                        "[Startup] Cleanup '{}': {} items removed",
                        task, count
                    )),
                    Err(e) => {
                        aaagent::logger::log(format!("[Startup] Cleanup '{}' failed: {}", task, e))
                    }
                }
            }
        });

        Ok(Self {
            config_resolver,
            session_store: Arc::new(FileSessionStore::new("data/sessions")?),
            stream_manager: Arc::new(StreamManager::new()),
            tree_store: Arc::new(aaagent::history::MemoryStore::new()),
        })
    }
}

pub fn create_router() -> Router {
    let state = AppState::new().expect("Failed to initialize app state");

    // Start background maintenance worker
    let maintenance_config = state
        .config_resolver
        .config_manager()
        .maintenance_config()
        .clone();
    aaagent::maintenance::start_maintenance_worker(maintenance_config);

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
    #[serde(default)]
    config: Option<ChatConfig>,
    #[serde(default)]
    temporary_config: Option<ChatConfig>,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    stream_id: String,
    resolved_config: ResolvedConfig,
}

#[derive(Debug, Serialize)]
struct ConfigResponse {
    resolved_config: ResolvedConfig,
    editable_config: ChatConfig,
}

// Placeholder handlers
mod sessions {
    use super::*;

    pub async fn list_sessions(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
        let sessions = state
            .session_store
            .list_sessions()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(json!({
            "sessions": sessions,
            "total": sessions.len()
        })))
    }

    pub async fn create_session(
        State(state): State<AppState>,
        Json(req): Json<Value>,
    ) -> Result<Json<Value>, ApiError> {
        use aaagent::history::Session;

        let name = req
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Session");
        let preset = req
            .get("preset")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        // Validate preset and resolve config
        let chat_config = ChatConfig {
            preset: preset.to_string(),
            system_prompt: req
                .get("system_prompt")
                .and_then(|v| v.as_str())
                .map(String::from),
            tools_enabled: true,
            intent: Default::default(),
            overrides: None,
        };

        let resolved = state
            .config_resolver
            .resolve(&chat_config)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        // Create session with tree store (using shared tree_store from state)
        let tree_store = state.tree_store.clone();
        let session_config = aaagent::history::SessionConfig {
            system_prompt: resolved.session.system_prompt.clone().into(),
            max_context_tokens: resolved.session.max_context_tokens,
            ..Default::default()
        };

        let mut session = Session::new(tree_store, session_config)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Set session name and metadata
        session.name = Some(name.to_string());
        session.metadata = Some(json!({
            "preset": preset,
            "resolved_config": resolved,
        }));

        // Save to file storage
        state
            .session_store
            .create_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(json!({
            "session_id": session.session_id,
            "name": session.name,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "resolved_config": resolved
        })))
    }

    pub async fn get_session(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        // Extract preset from metadata if available
        let preset = session
            .metadata
            .as_ref()
            .and_then(|m| m.get("preset"))
            .and_then(|p| p.as_str())
            .unwrap_or("general");

        Ok(Json(json!({
            "session_id": session.session_id,
            "name": session.name,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "preset": preset,
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
        aaagent::logger::log(format!("Chat request received for session: {}", session_id));

        // Validate request
        if req.message.trim().is_empty() {
            return Err(ApiError::BadRequest("message cannot be empty".to_string()));
        }

        // Load session from storage
        let stored_session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        // Get base resolved config from session metadata
        let base_resolved = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|c| serde_json::from_value::<ResolvedConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing resolved_config".to_string()))?;

        // If temporary_config is provided, resolve it and use that instead
        let resolved = if let Some(temp_config) = req.temporary_config {
            aaagent::logger::log(format!(
                "Using temporary_config: preset={}, overrides={:?}",
                temp_config.preset, temp_config.overrides
            ));

            let mut temp_resolved = state
                .config_resolver
                .resolve(&temp_config)
                .map_err(|e| ApiError::BadRequest(format!("Invalid temporary_config: {}", e)))?;

            // Preserve immutable session fields from base config
            temp_resolved.session = base_resolved.session.clone();

            temp_resolved
        } else if let Some(config) = req.config {
            // Validate config changes don't modify immutable fields
            state
                .config_resolver
                .validate_immutable_fields(&config, &base_resolved)
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;

            let mut resolved = state
                .config_resolver
                .resolve(&config)
                .map_err(|e| ApiError::BadRequest(format!("Invalid config: {}", e)))?;

            // Preserve immutable session fields
            resolved.session = base_resolved.session.clone();

            resolved
        } else {
            base_resolved
        };

        // Create stream
        let (stream_id, tx) = state.stream_manager.create_stream().await;
        aaagent::logger::log(format!(
            "Created stream: {} for session: {}",
            stream_id, session_id
        ));

        // Clone values for the background task
        let message = req.message.clone();
        let config_manager = state.config_resolver.clone();
        let stream_id_clone = stream_id.clone();
        let resolved_for_task = resolved.clone();
        let tree_store_for_task = state.tree_store.clone();

        // Spawn background task to run Agent
        tokio::spawn(async move {
            aaagent::logger::log(format!(
                "Starting agent chat for stream: {}",
                stream_id_clone
            ));

            let result = run_agent_chat(
                stored_session,
                message,
                resolved_for_task,
                config_manager,
                tree_store_for_task,
                tx.clone(),
            )
            .await;

            // If agent failed, send error to frontend
            match result {
                Ok(_) => {
                    aaagent::logger::log(format!(
                        "Agent chat completed successfully for stream: {}",
                        stream_id_clone
                    ));
                }
                Err(e) => {
                    aaagent::logger::log(format!(
                        "ERROR: Agent chat failed for stream {}: {}",
                        stream_id_clone, e
                    ));
                    aaagent::logger::log(format!("ERROR: Error details: {:?}", e));

                    // Send error message to frontend
                    let error_msg = format!("❌ Agent Error: {}\n\nDetails: {}", e, e);
                    aaagent::logger::log(format!("Sending error message to stream: {}", error_msg));

                    match tx
                        .send(aaagent::agent::AgentEvent::Content(error_msg.clone()))
                        .await
                    {
                        Ok(_) => {
                            aaagent::logger::log("Error message sent successfully".to_string())
                        }
                        Err(send_err) => aaagent::logger::log(format!(
                            "ERROR: Failed to send error message: {}",
                            send_err
                        )),
                    }

                    // Send done event
                    let _ = tx
                        .send(aaagent::agent::AgentEvent::Done {
                            total_usage: aaagent::llm::TokenUsage {
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
                    aaagent::logger::log("Sender dropped, channel closed cleanly".to_string());

                    // Wait for SSE to read messages
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                }
            }

            // TODO: Save updated session back to storage
            // For now, sessions are not persisted after chat
        });

        Ok(Json(ChatResponse {
            stream_id,
            resolved_config: resolved,
        }))
    }

    pub async fn get_path(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        // Load session
        let mut session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        // Attach shared store
        session.set_store(state.tree_store.clone());

        // Get path nodes from active leaf to root
        let leaf_id = session.active_leaf_id.clone();
        let nodes = state
            .tree_store
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
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

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
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        Ok(Json(json!({ "checkpoints": session.checkpoints })))
    }

    pub async fn get_system_prompt(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        Ok(Json(json!({
            "prompt": session.config.system_prompt.unwrap_or_default()
        })))
    }

    pub async fn get_config(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<ConfigResponse>, ApiError> {
        let session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        // Load resolved config from session metadata
        let resolved_config = session
            .metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|c| serde_json::from_value::<ResolvedConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing resolved_config".to_string()))?;

        // Load preset from metadata
        let preset = session
            .metadata
            .as_ref()
            .and_then(|m| m.get("preset"))
            .and_then(|p| p.as_str())
            .unwrap_or("general")
            .to_string();

        // Map resolved config back to editable ChatConfig
        let editable_config = ChatConfig {
            preset,
            system_prompt: None, // Immutable, shown separately
            tools_enabled: resolved_config.agent.tools_enabled,
            intent: ChatIntent {
                creativity: 0.5, // TODO: Reverse-map from temperature
                verbosity: match resolved_config.provider.max_tokens {
                    8192 => "short".to_string(),
                    16384 => "normal".to_string(),
                    32768 => "long".to_string(),
                    _ => "normal".to_string(),
                },
                rounds: resolved_config.agent.max_rounds,
            },
            overrides: None,
        };

        Ok(Json(ConfigResponse {
            resolved_config,
            editable_config,
        }))
    }

    pub async fn update_config(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(config): Json<ChatConfig>,
    ) -> Result<Json<ResolvedConfig>, ApiError> {
        // Load existing session
        let mut session = state
            .session_store
            .get_session(&session_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

        // Load existing resolved config
        let existing_resolved = session
            .metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|c| serde_json::from_value::<ResolvedConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing resolved_config".to_string()))?;

        // Check immutable fields
        state
            .config_resolver
            .validate_immutable_fields(&config, &existing_resolved)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        // Resolve new configuration
        let mut resolved = state
            .config_resolver
            .resolve(&config)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        // Preserve immutable fields from existing config
        resolved.session.system_prompt = existing_resolved.session.system_prompt;
        resolved.session.max_context_tokens = existing_resolved.session.max_context_tokens;

        // Update session metadata
        if let Some(ref mut metadata) = session.metadata {
            metadata["resolved_config"] =
                serde_json::to_value(&resolved).map_err(|e| ApiError::Internal(e.to_string()))?;
            metadata["preset"] = json!(config.preset);
        }
        session.updated_at = aaagent::history::node::now();

        // Save updated session
        state
            .session_store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(resolved))
    }
}

/// Run agent chat in background task
async fn run_agent_chat(
    session: aaagent::history::Session,
    message: String,
    resolved_config: ResolvedConfig,
    config_manager: Arc<ConfigResolver>,
    tree_store: Arc<aaagent::history::MemoryStore>,
    tx: tokio::sync::mpsc::Sender<aaagent::agent::AgentEvent>,
) -> anyhow::Result<()> {
    use aaagent::agent::{Agent, AgentConfig};
    use aaagent::llm::{LoopDetectorConfig, ToolRegistry};

    aaagent::logger::log("run_agent_chat: Reconstructing session with tree store".to_string());
    
    // Attach the shared store to the session loaded from storage
    let mut session = session;
    session.set_store(tree_store.clone());

    // Ensure session metadata exists in the tree store (in case of server restart)
    // This allows the tree store to know about the session's active leaf etc.
    let _ = tree_store.update_session(&session).await;

    aaagent::logger::log(format!(
        "run_agent_chat: Creating provider (model: {})",
        resolved_config.provider.model
    ));
    // Create provider from resolved config
    let provider =
        provider_factory::create_provider(&resolved_config, config_manager.config_manager())?;

    aaagent::logger::log("run_agent_chat: Creating tool registry".to_string());
    // Create tool registry
    let registry = ToolRegistry::new().register_all_builtin();

    aaagent::logger::log("run_agent_chat: Creating agent".to_string());
    // Create agent
    let mut agent = Agent::new(session, provider, registry);
    agent.set_config(AgentConfig {
        max_rounds: resolved_config.agent.max_rounds as usize,
        loop_detection: Some(LoopDetectorConfig::default()),
    });

    aaagent::logger::log(format!(
        "run_agent_chat: Starting chat with message: {}",
        message
    ));
    // Run chat with callback to stream events
    let result = agent
        .chat_with_callback(&message, |event| {
            let tx = tx.clone();
            async move {
                // Debug logging disabled for performance
                // aaagent::logger::log(format!("DEBUG: Sending event: {:?}", event));
                // Send event through channel (ignore if channel is closed)
                let _ = tx.send(event).await;
            }
        })
        .await;

    match result {
        Ok(_) => {
            aaagent::logger::log("run_agent_chat: Chat completed successfully".to_string());
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
        aaagent::logger::log(format!("SSE stream requested: {}", stream_id));

        // Take the stream from the manager
        let receiver = state
            .stream_manager
            .take_stream(&stream_id)
            .await
            .ok_or_else(|| {
                aaagent::logger::log(format!("ERROR: Stream {} not found", stream_id));
                ApiError::NotFound(format!("Stream {} not found", stream_id))
            })?;

        aaagent::logger::log(format!("SSE stream connection established: {}", stream_id));

        // Convert mpsc::Receiver to Stream
        let event_stream = ReceiverStream::new(receiver).map(|agent_event| {
            // Convert AgentEvent to SSE Event
            let (event_type, data) = match agent_event {
                aaagent::agent::AgentEvent::Content(content) => {
                    ("content", serde_json::json!({ "content": content }))
                }
                aaagent::agent::AgentEvent::Thinking(text) => {
                    ("thinking", serde_json::json!({ "text": text }))
                }
                aaagent::agent::AgentEvent::ToolCallsRequested { tool_calls } => (
                    "tool_calls",
                    serde_json::json!({ "tool_calls": tool_calls }),
                ),
                aaagent::agent::AgentEvent::ToolResult {
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
                aaagent::agent::AgentEvent::LoopDetected { detection } => (
                    "loop_detected",
                    serde_json::json!({ "detection": format!("{:?}", detection) }),
                ),
                aaagent::agent::AgentEvent::CheckpointCreated { node_id, strategy } => (
                    "checkpoint",
                    serde_json::json!({ "node_id": node_id, "strategy": strategy }),
                ),
                aaagent::agent::AgentEvent::Done {
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
