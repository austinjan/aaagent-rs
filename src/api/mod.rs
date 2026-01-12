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

use aaagent::config::{
    AgentConfig, ChatConfig, ChatIntent, ConfigResolver, ProviderConfig, ResolvedConfig,
    SessionConfig,
};
use aaagent::storage::{FileSessionStore, SessionStore};

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub session_store: Arc<dyn SessionStore>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        // Ensure data directory exists
        std::fs::create_dir_all("data/sessions")?;

        Ok(Self {
            config_resolver: Arc::new(ConfigResolver::new()?),
            session_store: Arc::new(FileSessionStore::new("data/sessions")?),
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
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
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

    pub async fn list_sessions() -> Json<Value> {
        // TODO: Load actual sessions from storage
        Json(json!({
            "sessions": [
                {
                    "session_id": "session-1",
                    "name": "General Conversation",
                    "created_at": 1704672000000_i64,
                    "updated_at": 1704758400000_i64,
                    "preset": "general",
                    "message_count": 42
                },
                {
                    "session_id": "session-2",
                    "name": "Coding Project",
                    "created_at": 1704585600000_i64,
                    "updated_at": 1704672000000_i64,
                    "preset": "coding",
                    "message_count": 128
                },
                {
                    "session_id": "session-3",
                    "name": "Research Task",
                    "created_at": 1704499200000_i64,
                    "updated_at": 1704585600000_i64,
                    "preset": "research",
                    "message_count": 87
                }
            ],
            "total": 3
        }))
    }

    pub async fn create_session(
        State(state): State<AppState>,
        Json(req): Json<Value>,
    ) -> Result<Json<Value>, ApiError> {
        // TODO: Create actual session in storage
        let session_id = format!("session-{}", ulid::Ulid::new());
        let name = req
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("New Session");
        let preset = req
            .get("preset")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        // Validate preset
        let config = ChatConfig {
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
            .resolve(&config)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        Ok(Json(json!({
            "session_id": session_id,
            "name": name,
            "created_at": chrono::Utc::now().timestamp_millis(),
            "updated_at": chrono::Utc::now().timestamp_millis(),
            "resolved_config": resolved
        })))
    }

    pub async fn get_session(Path(session_id): Path<String>) -> Json<Value> {
        // TODO: Load actual session from storage
        Json(json!({
            "session_id": session_id,
            "name": "Example Session",
            "created_at": 1704672000000_i64,
            "updated_at": 1704758400000_i64,
            "preset": "general",
            "message_count": 42,
            "root_node_id": "node-root",
            "active_leaf_id": "node-leaf-123"
        }))
    }

    pub async fn chat(
        Path(_session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<ChatRequest>,
    ) -> Result<Json<ChatResponse>, ApiError> {
        // Validate request
        if req.message.trim().is_empty() {
            return Err(ApiError::BadRequest("message cannot be empty".to_string()));
        }

        // Use temporary config if provided, otherwise use persistent config
        let config = req
            .temporary_config
            .or(req.config)
            .unwrap_or_else(|| ChatConfig {
                preset: "general".to_string(),
                system_prompt: None,
                tools_enabled: true,
                intent: Default::default(),
                overrides: None,
            });

        // Resolve configuration
        let resolved = state
            .config_resolver
            .resolve(&config)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        // TODO: Actually process the chat request with Agent
        // For now, just return the resolved config
        let stream_id = format!("stream-{}", ulid::Ulid::new());

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
        Path(_session_id): Path<String>,
    ) -> Result<Json<ConfigResponse>, ApiError> {
        // TODO: Load actual session from storage and retrieve config from metadata
        // For now, return a placeholder resolved config

        // This would be loaded from session.metadata["resolved_config"]
        let resolved_config = ResolvedConfig {
            provider: ProviderConfig {
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 16384,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
            },
            agent: AgentConfig {
                max_rounds: 30,
                tools_enabled: true,
            },
            session: SessionConfig {
                system_prompt: "You are a helpful, friendly assistant.".to_string(),
                max_context_tokens: 200000,
            },
        };

        // Map resolved config back to editable ChatConfig
        // Note: We derive intent fields from resolved values
        let editable_config = ChatConfig {
            preset: "general".to_string(),
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
        Path(_session_id): Path<String>,
        State(state): State<AppState>,
        Json(config): Json<ChatConfig>,
    ) -> Result<Json<ResolvedConfig>, ApiError> {
        // Validate that system_prompt is not being changed
        // TODO: Load existing session config from metadata
        let existing_resolved = ResolvedConfig {
            provider: ProviderConfig {
                model: "gpt-5-mini".to_string(),
                temperature: 1.0,
                max_tokens: 16384,
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
            },
            agent: AgentConfig {
                max_rounds: 30,
                tools_enabled: true,
            },
            session: SessionConfig {
                system_prompt: "You are a helpful, friendly assistant.".to_string(),
                max_context_tokens: 200000,
            },
        };

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

        // TODO: Save resolved config to session.metadata["resolved_config"]

        Ok(Json(resolved))
    }
}

mod sse {
    use axum::response::sse::Event;
    use futures::stream;
    use std::convert::Infallible;

    pub async fn stream() -> impl axum::response::IntoResponse {
        // Placeholder
        axum::response::sse::Sse::new(stream::empty::<Result<Event, Infallible>>())
    }
}
