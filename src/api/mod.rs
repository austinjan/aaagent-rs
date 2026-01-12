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
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all("data/sessions")?;

        Ok(Self {
            config_resolver: Arc::new(ConfigResolver::new()?),
            session_store: Arc::new(FileSessionStore::new("data/sessions")?),
            stream_manager: Arc::new(StreamManager::new()),
        })
    }
}

pub fn create_router() -> Router {
    let state = AppState::new().expect("Failed to initialize app state");

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
        use aaagent::history::{MemoryStore, Session};
        use std::sync::Arc;

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

        // Create session with tree store (using MemoryStore for now)
        let tree_store = Arc::new(MemoryStore::new());
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

        // Get resolved config from session metadata
        let resolved = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("resolved_config"))
            .and_then(|c| serde_json::from_value::<ResolvedConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing resolved_config".to_string()))?;

        // Create stream
        let (stream_id, tx) = state.stream_manager.create_stream().await;
        aaagent::logger::log(format!(
            "Created stream: {} for session: {}",
            stream_id, session_id
        ));

        // Clone values for the background task
        let message = req.message.clone();
        let config_manager = state.config_resolver.clone();
        let _session_store = state.session_store.clone();
        let stream_id_clone = stream_id.clone();
        let resolved_for_task = resolved.clone();

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
                tx,
            )
            .await;

            // If agent failed, we should still save the session if possible
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

                    // Note: tx was already moved into run_agent_chat, so we can't send error here
                    // The SSE stream will close and frontend will show generic error
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

    pub async fn get_path() -> Json<Value> {
        Json(json!({"nodes": []}))
    }

    pub async fn get_metadata() -> Json<Value> {
        Json(json!({"total_nodes": 0}))
    }

    pub async fn get_checkpoints() -> Json<Value> {
        Json(json!({"checkpoints": []}))
    }

    pub async fn get_system_prompt() -> Json<Value> {
        Json(json!({"prompt": ""}))
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
    tx: tokio::sync::mpsc::Sender<aaagent::agent::AgentEvent>,
) -> anyhow::Result<()> {
    use aaagent::agent::Agent;
    use aaagent::history::MemoryStore;
    use aaagent::llm::ToolRegistry;

    aaagent::logger::log("run_agent_chat: Creating tree store".to_string());
    // Create tree store for the session
    // Note: The session loaded from file storage doesn't have a TreeStore attached
    // We need to create one and populate it from the session data
    let tree_store = Arc::new(MemoryStore::new());

    aaagent::logger::log("run_agent_chat: Creating new session".to_string());
    // TODO: Properly reconstruct session with tree store
    // For now, create a new session - this is a limitation we need to fix
    let session =
        aaagent::history::Session::new(tree_store.clone(), session.config.clone()).await?;

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
            // Send error message to frontend before returning error
            let error_msg = format!("❌ Agent Error: {}\n\nDetails logged in app.log", e);
            let _ = tx
                .send(aaagent::agent::AgentEvent::Content(error_msg))
                .await;
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
