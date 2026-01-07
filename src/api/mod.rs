use axum::{
    Router,
    routing::{get, post},
    Json,
};
use serde_json::{json, Value};

#[cfg(feature = "dev-server")]
use tower_http::cors::CorsLayer;

pub fn create_router() -> Router {
    let router = Router::new()
        // API routes (under /api prefix)
        .nest("/api", api_routes())

        // Static files (fallback for everything else)
        .fallback(crate::web::static_handler);

    // Add CORS middleware only in dev mode
    #[cfg(feature = "dev-server")]
    let router = router.layer(CorsLayer::permissive());

    router
}

fn api_routes() -> Router {
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

// Placeholder handlers
mod sessions {
    use super::*;

    pub async fn chat() -> Json<Value> {
        Json(json!({"status": "not implemented"}))
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
