use axum::{
    Router,
    routing::{get, post},
    Json,
    extract::{Path, State},
    http::{StatusCode, header::{AUTHORIZATION, HeaderName}},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;

#[cfg(feature = "dev-server")]
use tower_http::cors::CorsLayer;

use aaagent::config::{ChatConfig, ConfigResolver, ResolvedConfig};

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            config_resolver: Arc::new(ConfigResolver::new()?),
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
        .route("/sessions/:session_id/chat", post(sessions::chat))
        .route("/sessions/:session_id/stream/:stream_id", get(sse::stream))
        .route("/sessions/:session_id/path", get(sessions::get_path))
        .route("/sessions/:session_id/path/metadata", get(sessions::get_metadata))
        .route("/sessions/:session_id/checkpoints", get(sessions::get_checkpoints))
        .route("/sessions/:session_id/system-prompt", get(sessions::get_system_prompt))
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

// Placeholder handlers
mod sessions {
    use super::*;

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
        let config = req.temporary_config.or(req.config).unwrap_or_else(|| ChatConfig {
            preset: "general".to_string(),
            system_prompt: None,
            tools_enabled: true,
            intent: Default::default(),
            overrides: None,
        });

        // Resolve configuration
        let resolved = state.config_resolver.resolve(&config)
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
