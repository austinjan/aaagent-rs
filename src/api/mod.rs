use axum::{
    extract::{Path, Query, State},
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

use crate::agent::{AgentFactory, AgentRuntime, SessionManager, SubAgentRegistry};
use crate::config::{ConfigResolver, SessionConfig};
use crate::history::{SessionConfig as HistorySessionConfig, TreeStore};
use crate::llm::LLMProvider;

pub mod event_bus;
mod provider_factory;
mod stream_manager;

pub use event_bus::{AgentEventEnvelope, GlobalEventBus};
use stream_manager::StreamManager;

// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config_resolver: Arc<ConfigResolver>,
    pub store: Arc<crate::history::JSONLStore>,
    pub stream_manager: Arc<StreamManager>,
    pub event_bus: Arc<GlobalEventBus>,
    pub session_manager: Arc<SessionManager>,
    pub agent_factory: Arc<AgentFactory>,
    pub runtime: Arc<AgentRuntime>,
    pub registry: Arc<SubAgentRegistry>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        // Ensure data directories exist
        std::fs::create_dir_all("data")?;
        std::fs::create_dir_all("data/temp")?;
        std::fs::create_dir_all("data/sub-agent-sessions")?;

        // Load configuration
        let config_resolver = Arc::new(ConfigResolver::new()?);

        // Initialize JSONL store with lazy loading
        crate::logger::log(format!(
            "[Startup] Initializing JSONL store (sync_every: {})",
            std::env::var("AAAGENT_JSONL_SYNC_EVERY").unwrap_or_else(|_| "1".to_string())
        ));
        let sync_every_writes = std::env::var("AAAGENT_JSONL_SYNC_EVERY")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1);
        let store = Arc::new(
            crate::history::JSONLStore::new_with_sync_every("data".into(), sync_every_writes)
                .await?,
        );

        // Initialize separate storage for sub-agent sessions
        let subagent_store = Arc::new(
            crate::history::JSONLStore::new_with_sync_every(
                "data/sub-agent-sessions".into(),
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

        // Initialize sub-agent infrastructure
        std::fs::create_dir_all("data/sessions")?;
        std::fs::create_dir_all("data/subagents")?;

        let runtime = Arc::new(AgentRuntime::new());
        let event_bus = Arc::new(GlobalEventBus::new());
        let registry = Arc::new(SubAgentRegistry::new("data/subagents/registry.json".into()));

        // Create SessionManager with persistence
        let session_manager = Arc::new(SessionManager::new(
            Arc::clone(&store) as Arc<dyn TreeStore>,
            HistorySessionConfig::default(),
            Some("data/sessions".into()),
        ));

        // Create AgentFactory with provider factory
        let provider_factory = Arc::new(|| provider_factory::create_default_provider());

        let base_tools = crate::llm::ToolRegistry::new().register_all_builtin();

        let agent_factory = Arc::new(AgentFactory::new(
            provider_factory,
            base_tools,
            Arc::clone(&runtime),
            Arc::clone(&registry),
            Arc::clone(&event_bus),
            Arc::clone(&store) as Arc<dyn TreeStore>,
            Arc::clone(&subagent_store) as Arc<dyn TreeStore>,
            8, // max_concurrent sub-agents
        ));

        crate::logger::log("[Startup] Sub-agent system initialized".to_string());

        Ok(Self {
            config_resolver,
            store,
            stream_manager: Arc::new(StreamManager::new()),
            event_bus: Arc::clone(&event_bus),
            session_manager,
            agent_factory,
            runtime,
            registry,
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

    // Start inject listener for sub-agent completions
    let _inject_listener_handle = crate::agent::start_inject_listener(
        Arc::clone(&state.event_bus),
        Arc::clone(&state.session_manager),
        Arc::clone(&state.agent_factory),
    );
    crate::logger::log("[Startup] Inject listener started".to_string());

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
        .route("/skills", get(list_skills))
        .route("/providers/status", get(providers_status))
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions", post(sessions::create_session))
        .route("/sessions/:session_id", get(sessions::get_session))
        .route(
            "/sessions/:session_id/archive",
            axum::routing::patch(sessions::archive_session),
        )
        .route(
            "/sessions/:session_id/rename",
            axum::routing::patch(sessions::rename_session),
        )
        .route(
            "/sessions/:session_id/tags",
            axum::routing::patch(sessions::update_tags),
        )
        .route(
            "/sessions/:session_id/ai-rename",
            post(sessions::ai_rename_session),
        )
        .route("/sessions/:session_id/chat", post(sessions::chat))
        .route("/sessions/:session_id/stream/:stream_id", get(sse::stream))
        .route("/sessions/:session_id/events", get(sse::stream_events))
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
            "/sessions/:session_id/checkpoints",
            post(sessions::create_checkpoint),
        )
        .route(
            "/sessions/:session_id/checkpoints/preview",
            post(sessions::preview_checkpoint),
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
        // Branch operations
        .route(
            "/sessions/:session_id/switch-branch",
            post(sessions::switch_branch),
        )
        .route(
            "/sessions/:session_id/branch-from",
            post(sessions::create_branch),
        )
        // Tree visualization
        .route("/sessions/:session_id/tree", get(sessions::get_tree))
        .route(
            "/sessions/:session_id/tree-stats",
            get(sessions::get_tree_stats),
        )
        // Node collapse/expand
        .route(
            "/sessions/:session_id/nodes/:node_id/collapse",
            post(sessions::collapse_node),
        )
        .route(
            "/sessions/:session_id/nodes/:node_id/expand",
            post(sessions::expand_node),
        )
        .route(
            "/sessions/:session_id/nodes/:node_id/collapse-tool-group",
            post(sessions::collapse_tool_group),
        )
        .route(
            "/sessions/:session_id/collapsed-state",
            get(sessions::get_collapsed_state),
        )
        .route(
            "/sessions/:session_id/collapse-all-tools",
            post(sessions::collapse_all_tools),
        )
        .route(
            "/sessions/:session_id/expand-all",
            post(sessions::expand_all),
        )
        // Context as markdown string
        .route(
            "/sessions/:session_id/context-string",
            get(sessions::get_context_string),
        )
        // Session metrics (active subscribers, event counts)
        .route("/sessions/:session_id/metrics", get(sessions::get_metrics))
        // Session export (Markdown, JSON)
        .route(
            "/sessions/:session_id/export",
            get(sessions::export_session),
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

// Provider availability endpoint - reports which providers have API keys configured
async fn providers_status(State(state): State<AppState>) -> Json<Value> {
    let cm = state.config_resolver.config_manager();
    Json(json!({
        "openai": cm.has_api_key("openai"),
        "anthropic": cm.has_api_key("anthropic"),
        "google": cm.has_api_key("google"),
    }))
}

// List all skills (loaded + errors)
async fn list_skills() -> Json<Value> {
    let app_home = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".aaagent");
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let manager = crate::skills::SkillsManager::new(&app_home, &cwd);

    // Build loaded skills list
    let loaded: Vec<Value> = manager
        .skills()
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "description": s.description,
                "path": s.path.display().to_string(),
                "scope": format!("{:?}", s.scope),
                "user_invocable": s.invocation.user_invocable,
                "model_invocable": !s.invocation.disable_model_invocation,
            })
        })
        .collect();

    // Build errors list with categorization
    let errors: Vec<Value> = manager
        .errors()
        .iter()
        .map(|e| {
            let category = if e.contains("not eligible") {
                "eligibility"
            } else if e.contains("Invalid YAML") || e.contains("Invalid metadata") {
                "parse_error"
            } else if e.contains("Cannot read") {
                "file_error"
            } else {
                "unknown"
            };
            json!({
                "error": e,
                "category": category,
            })
        })
        .collect();

    Json(json!({
        "loaded": {
            "count": loaded.len(),
            "skills": loaded,
        },
        "errors": {
            "count": errors.len(),
            "details": errors,
        },
        "summary": format!("{} skills loaded, {} errors", loaded.len(), errors.len()),
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

    #[derive(Deserialize)]
    pub struct ListSessionsQuery {
        tags: Option<String>,           // Comma-separated tags
        name: Option<String>,           // Name search (case-insensitive contains)
        created_after: Option<i64>,     // Unix timestamp
        created_before: Option<i64>,    // Unix timestamp
        include_archived: Option<bool>, // Include archived sessions (default: false)
    }

    pub async fn list_sessions(
        State(state): State<AppState>,
        Query(query): Query<ListSessionsQuery>,
    ) -> Result<Json<Value>, ApiError> {
        let sessions = state
            .store
            .list_sessions()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Parse tags filter
        let tag_filters: Vec<String> = query
            .tags
            .as_ref()
            .map(|t| t.split(',').map(|s| s.trim().to_lowercase()).collect())
            .unwrap_or_default();

        // Filter sessions
        let include_archived = query.include_archived.unwrap_or(false);
        let filtered_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| {
                // Filter out archived unless explicitly requested
                if s.archived && !include_archived {
                    return false;
                }

                // Filter by tags (session must have ALL specified tags)
                if !tag_filters.is_empty() {
                    let session_tags_lower: Vec<String> =
                        s.tags.iter().map(|t| t.to_lowercase()).collect();
                    if !tag_filters
                        .iter()
                        .all(|filter_tag| session_tags_lower.contains(filter_tag))
                    {
                        return false;
                    }
                }

                // Filter by name (case-insensitive contains)
                if let Some(name_query) = &query.name {
                    if let Some(session_name) = &s.name {
                        if !session_name
                            .to_lowercase()
                            .contains(&name_query.to_lowercase())
                        {
                            return false;
                        }
                    } else {
                        return false; // No name, doesn't match
                    }
                }

                // Filter by created_after
                if let Some(after) = query.created_after {
                    if s.created_at < after {
                        return false;
                    }
                }

                // Filter by created_before
                if let Some(before) = query.created_before {
                    if s.created_at > before {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Convert to session summaries
        let summaries: Vec<_> = filtered_sessions
            .iter()
            .map(|s| {
                json!({
                    "session_id": s.session_id,
                    "name": s.name,
                    "tags": s.tags,
                    "created_at": s.created_at,
                    "updated_at": s.updated_at,
                    "message_count": s.stats.total_nodes,
                    "archived": s.archived,
                })
            })
            .collect();

        Ok(Json(json!({
            "sessions": summaries,
            "total": summaries.len()
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

    pub async fn archive_session(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Archive session request for: {}", session_id));

        // Archive the session
        state
            .store
            .archive_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!("Session archived successfully: {}", session_id));

        Ok(Json(json!({
            "success": true,
            "session_id": session_id,
            "archived": true
        })))
    }

    #[derive(Debug, Deserialize)]
    pub struct RenameSessionRequest {
        name: String,
    }

    pub async fn rename_session(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<RenameSessionRequest>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Rename session request for: {}", session_id));

        // Validate name
        let name = req.name.trim();
        if name.is_empty() {
            return Err(ApiError::BadRequest(
                "Session name cannot be empty".to_string(),
            ));
        }

        // Load and update session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.name = Some(name.to_string());
        session.updated_at = crate::history::node::now();

        // Save updated session
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!("Session renamed successfully: {}", session_id));

        Ok(Json(json!({
            "success": true,
            "session_id": session_id,
            "name": name
        })))
    }

    #[derive(Deserialize)]
    pub struct UpdateTagsRequest {
        tags: Vec<String>,
    }

    pub async fn update_tags(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<UpdateTagsRequest>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Update tags request for: {}", session_id));

        // Load and update session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session not found: {}", session_id)))?;

        // Update tags
        session.tags = req.tags;
        session.updated_at = crate::history::node::now();

        // Save
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!("Session tags updated: {}", session_id));

        Ok(Json(json!({
            "success": true,
            "session_id": session_id,
            "tags": session.tags
        })))
    }

    pub async fn ai_rename_session(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("AI rename session request for: {}", session_id));

        // Load session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.set_store(state.store.clone());

        // Get conversation context (first 10 messages or up to 2000 tokens)
        let context = session
            .get_context()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Build a summary of the conversation for the LLM
        let mut conversation_text = String::new();
        for msg in context.iter().take(10) {
            conversation_text.push_str(&format!("{:?}: {}\n", msg.role, msg.content));
        }

        // Ask LLM to generate a short session name
        let prompt = format!(
            "Based on the following conversation, provide a very short (3-6 words maximum) descriptive name for this chat session. \
            Only respond with the session name, nothing else.\n\nConversation:\n{}",
            conversation_text.chars().take(1000).collect::<String>()
        );

        let name = provider_factory::quick_prompt(state.config_resolver.config_manager(), &prompt)
            .await
            .map_err(|e| ApiError::Internal(format!("AI naming failed: {}", e)))?;

        // Extract the generated name and clean it up
        let mut name = name.trim().to_string();

        // Remove quotes if present
        if name.starts_with('"') && name.ends_with('"') {
            name = name[1..name.len() - 1].to_string();
        }
        if name.starts_with('\'') && name.ends_with('\'') {
            name = name[1..name.len() - 1].to_string();
        }

        // Truncate if too long
        if name.len() > 60 {
            name = format!("{}...", &name[..60]);
        }

        // Update session
        session.name = Some(name.clone());
        session.updated_at = crate::history::node::now();

        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!(
            "Session AI-renamed successfully: {} -> {}",
            session_id, name
        ));

        Ok(Json(json!({
            "success": true,
            "session_id": session_id,
            "name": name
        })))
    }

    // Handler of /chat endpoint
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
        let mut stored_session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Set session name to first user message if not already set
        if stored_session.name.is_none()
            || stored_session
                .name
                .as_ref()
                .map_or(false, |n| n.is_empty() || n == "New Session")
        {
            // Truncate message to 50 characters for session name
            let name = if req.message.len() > 50 {
                format!("{}...", &req.message[..50])
            } else {
                req.message.clone()
            };
            stored_session.name = Some(name);
            stored_session.updated_at = crate::history::node::now();

            // Save updated session with name
            state
                .store
                .update_session(&stored_session)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }

        let session_config = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("session_config"))
            .and_then(|c| serde_json::from_value::<SessionConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing session_config".to_string()))?;

        // Generate run ID for this agent execution
        let run_id = ulid::Ulid::new().to_string();

        crate::logger::log(format!(
            "Starting agent chat run: {} for session: {}",
            run_id, session_id
        ));

        // Clone values for the background task
        let message = req.message.clone();
        let config_manager = state.config_resolver.clone();
        let session_config_for_task = session_config.clone();
        let store_for_task = state.store.clone();
        let event_bus = state.event_bus.clone();
        let session_id_for_task = session_id.clone();
        let run_id_for_task = run_id.clone();

        // Spawn background task to run Agent
        tokio::spawn(async move {
            crate::logger::log(format!("Starting agent chat for run: {}", run_id_for_task));

            let result = run_agent_chat(
                stored_session,
                message,
                session_config_for_task,
                config_manager,
                store_for_task.clone(),
                event_bus,
                session_id_for_task,
                run_id_for_task.clone(),
            )
            .await;

            // If agent failed, log error
            match result {
                Ok(mut updated_session) => {
                    crate::logger::log(format!(
                        "Agent chat completed successfully for run: {}",
                        run_id_for_task
                    ));

                    // Auto-collapse tool groups for any new assistant messages with tool calls
                    updated_session.set_store(store_for_task.clone());
                    if let Err(e) = updated_session.auto_collapse_all_tool_groups().await {
                        crate::logger::log(format!(
                            "Warning: Auto-collapse failed: {}",
                            e
                        ));
                    }

                    // Save updated session back to storage
                    if let Err(e) = store_for_task.update_session(&updated_session).await {
                        crate::logger::log(format!(
                            "ERROR: Failed to save session after chat: {}",
                            e
                        ));
                    } else {
                        crate::logger::log(format!(
                            "Session {} saved successfully (active_leaf: {}, stats: {} nodes)",
                            updated_session.session_id,
                            updated_session.active_leaf_id,
                            updated_session.stats.total_nodes
                        ));
                    }
                }
                Err(e) => {
                    crate::logger::log(format!(
                        "ERROR: Agent chat failed for run {}: {}",
                        run_id_for_task, e
                    ));
                    crate::logger::log(format!("ERROR: Error details: {:?}", e));
                }
            }
        });

        // Return run_id as stream_id for backward compatibility with frontend
        Ok(Json(ChatResponse { stream_id: run_id }))
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

        // Convert nodes to JSON with checkpoint information
        let nodes_json: Vec<Value> = nodes
            .iter()
            .map(|node| {
                let has_checkpoint = session.checkpoints.contains_key(&node.node_id);
                let checkpoint_data = session.checkpoints.get(&node.node_id);

                json!({
                    "node_id": node.node_id,
                    "session_id": node.session_id,
                    "parent_id": node.parent_id,
                    "kind": format!("{:?}", node.kind),
                    "role": node.role.as_ref().map(|r| format!("{:?}", r)),
                    "content_type": format!("{:?}", node.content_type),
                    "content": node.content,
                    "created_at": node.created_at,
                    "seq": node.seq,
                    "flags": node.flags,
                    "tool_call_id": node.tool_call_id,
                    "tool_calls": node.tool_calls,
                    "has_checkpoint": has_checkpoint,
                    "checkpoint": checkpoint_data.map(|cp| json!({
                        "summary": &cp.summary,
                        "strategy": &cp.strategy,
                        "created_at": cp.created_at,
                        "stats": cp.stats.as_ref().map(|s| json!({
                            "nodes_covered": s.nodes_covered,
                            "total_tokens": s.total_tokens,
                            "summary_tokens": s.summary_tokens,
                            "compression_ratio": s.compression_ratio,
                        })),
                    })),
                })
            })
            .collect();

        Ok(Json(json!({ "nodes": nodes_json })))
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
            "total_input_tokens": session.stats.total_input_tokens,
            "total_output_tokens": session.stats.total_output_tokens,
            "total_cached_tokens": session.stats.total_cached_tokens,
            "total_tokens": session.stats.total_tokens(),
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

    /// Query parameters for context-string endpoint
    #[derive(Debug, Deserialize)]
    pub struct ContextStringQuery {
        /// Leaf node ID (defaults to active_leaf_id if not provided)
        pub leaf_id: Option<String>,
        /// Strategy: "full", "default", or "aggressive"
        #[serde(default)]
        pub strategy: ContextStringStrategyParam,
    }

    /// Strategy parameter for context-string endpoint
    #[derive(Debug, Clone, Copy, Default, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ContextStringStrategyParam {
        Full,
        #[default]
        Default,
        Aggressive,
    }

    impl From<ContextStringStrategyParam> for crate::history::ContextStringStrategy {
        fn from(param: ContextStringStrategyParam) -> Self {
            match param {
                ContextStringStrategyParam::Full => crate::history::ContextStringStrategy::Full,
                ContextStringStrategyParam::Default => {
                    crate::history::ContextStringStrategy::Default
                }
                ContextStringStrategyParam::Aggressive => {
                    crate::history::ContextStringStrategy::Aggressive
                }
            }
        }
    }

    /// Get context as a markdown-formatted string
    pub async fn get_context_string(
        Path(session_id): Path<String>,
        Query(query): Query<ContextStringQuery>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        // Load session
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| {
                eprintln!("[API] Failed to load session {}: {}", session_id, e);
                ApiError::Internal(format!("Failed to load session: {}", e))
            })?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Use provided leaf_id or default to active_leaf_id
        let leaf_id = query
            .leaf_id
            .unwrap_or_else(|| session.active_leaf_id.clone());

        eprintln!(
            "[API] Getting context string for session {} from leaf {} with strategy {:?}",
            session_id, leaf_id, query.strategy
        );

        // Convert strategy
        let strategy: crate::history::ContextStringStrategy = query.strategy.into();

        // Get context string
        let content = session
            .get_context_string_from(leaf_id.clone(), strategy)
            .await
            .map_err(|e| {
                eprintln!(
                    "[API] Failed to get context string from leaf {}: {}",
                    leaf_id, e
                );
                ApiError::Internal(format!("Failed to get context string: {}", e))
            })?;

        Ok(Json(json!({
            "session_id": session_id,
            "leaf_id": leaf_id,
            "strategy": format!("{:?}", query.strategy).to_lowercase(),
            "content": content
        })))
    }

    /// Request body for creating a checkpoint
    #[derive(Debug, Deserialize)]
    pub struct CreateCheckpointRequest {
        /// Node ID where the checkpoint will be created
        pub target_node_id: String,
        /// Compression strategy: "balanced", "aggressive", or "custom"
        #[serde(default)]
        pub strategy: crate::agent::CompressionStrategy,
        /// Custom prompt (only used when strategy is "custom")
        #[serde(skip_serializing_if = "Option::is_none")]
        pub custom_prompt: Option<String>,
        /// Use main provider instead of quick provider for higher quality
        #[serde(default)]
        pub use_main_provider: bool,
    }

    /// Create a checkpoint at a specific node
    pub async fn create_checkpoint(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<CreateCheckpointRequest>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!(
            "Create checkpoint request for session: {}, node: {}, strategy: {:?}",
            session_id, req.target_node_id, req.strategy
        ));

        // Load session
        let stored_session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Get session config
        let session_config = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("session_config"))
            .and_then(|c| serde_json::from_value::<SessionConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing session_config".to_string()))?;

        // Create provider
        let provider = provider_factory::create_provider(
            &session_config,
            state.config_resolver.config_manager(),
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Create agent
        let mut session = stored_session;
        session.set_store(state.store.clone());

        let registry = crate::llm::ToolRegistry::new();
        let mut agent = crate::agent::Agent::new(session, provider, registry);

        // Optionally set quick provider
        if let Ok(quick_provider) =
            provider_factory::create_quick_provider(state.config_resolver.config_manager())
        {
            agent.set_quick_provider(quick_provider);
        }

        // Create checkpoint options
        let options = crate::agent::CheckpointOptions {
            strategy: req.strategy,
            custom_prompt: req.custom_prompt,
            use_main_provider: req.use_main_provider,
        };

        // Create checkpoint
        let result = agent
            .create_checkpoint(req.target_node_id, options)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!(
            "Checkpoint created: {} (compression ratio: {:.1}%)",
            result.node_id,
            result.stats.compression_ratio * 100.0
        ));

        Ok(Json(json!({
            "checkpoint_id": result.node_id,
            "summary": result.summary,
            "stats": {
                "nodes_covered": result.stats.nodes_covered,
                "original_tokens": result.stats.total_tokens,
                "summary_tokens": result.stats.summary_tokens,
                "compression_ratio": result.stats.compression_ratio,
            }
        })))
    }

    /// Preview a checkpoint without creating it
    pub async fn preview_checkpoint(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<CreateCheckpointRequest>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!(
            "Preview checkpoint request for session: {}, node: {}, strategy: {:?}",
            session_id, req.target_node_id, req.strategy
        ));

        // Load session
        let stored_session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Get session config
        let session_config = stored_session
            .metadata
            .as_ref()
            .and_then(|m| m.get("session_config"))
            .and_then(|c| serde_json::from_value::<SessionConfig>(c.clone()).ok())
            .ok_or_else(|| ApiError::Internal("Session missing session_config".to_string()))?;

        // Create provider
        let provider = provider_factory::create_provider(
            &session_config,
            state.config_resolver.config_manager(),
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Create agent
        let mut session = stored_session;
        session.set_store(state.store.clone());

        let registry = crate::llm::ToolRegistry::new();
        let mut agent = crate::agent::Agent::new(session, provider, registry);

        // Optionally set quick provider
        if let Ok(quick_provider) =
            provider_factory::create_quick_provider(state.config_resolver.config_manager())
        {
            agent.set_quick_provider(quick_provider);
        }

        // Create checkpoint options
        let options = crate::agent::CheckpointOptions {
            strategy: req.strategy,
            custom_prompt: req.custom_prompt,
            use_main_provider: req.use_main_provider,
        };

        // Preview checkpoint (doesn't save)
        let result = agent
            .preview_checkpoint(req.target_node_id, options)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        Ok(Json(json!({
            "node_id": result.node_id,
            "summary": result.summary,
            "stats": {
                "nodes_covered": result.stats.nodes_covered,
                "original_tokens": result.stats.total_tokens,
                "estimated_summary_tokens": result.stats.summary_tokens,
                "estimated_compression_ratio": result.stats.compression_ratio,
            }
        })))
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

    // ===== Branch Operations =====

    #[derive(Debug, Deserialize)]
    pub struct SwitchBranchRequest {
        node_id: String,
    }

    #[derive(Debug, Serialize)]
    pub struct SwitchBranchResponse {
        success: bool,
        active_leaf_id: String,
        path: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }

    pub async fn switch_branch(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<SwitchBranchRequest>,
    ) -> Result<Json<SwitchBranchResponse>, ApiError> {
        crate::logger::log(format!(
            "Switch branch request for session: {}, node: {}",
            session_id, req.node_id
        ));

        // Load session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Verify the node exists
        let node = state
            .store
            .get_node(req.node_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Node {} not found", req.node_id)))?;

        // Verify the node belongs to this session
        if node.session_id != session_id {
            return Err(ApiError::BadRequest(
                "Node does not belong to this session".to_string(),
            ));
        }

        // Update the active leaf ID
        session.active_leaf_id = req.node_id.clone();
        session.updated_at = crate::history::node::now();

        // Save updated session
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Get the path from the new active leaf to root
        let path_nodes = state
            .store
            .get_path_to_root_internal(req.node_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let path: Vec<String> = path_nodes.iter().rev().map(|n| n.node_id.clone()).collect();

        crate::logger::log(format!("Branch switched successfully to: {}", req.node_id));

        Ok(Json(SwitchBranchResponse {
            success: true,
            active_leaf_id: req.node_id,
            path,
            error: None,
        }))
    }

    #[derive(Debug, Deserialize)]
    pub struct CreateBranchRequest {
        from_node_id: String,
    }

    #[derive(Debug, Serialize)]
    pub struct CreateBranchResponse {
        success: bool,
        new_leaf_id: String,
        branch_point_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }

    pub async fn create_branch(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
        Json(req): Json<CreateBranchRequest>,
    ) -> Result<Json<CreateBranchResponse>, ApiError> {
        crate::logger::log(format!(
            "Create branch request for session: {}, from node: {}",
            session_id, req.from_node_id
        ));

        // Load session
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Verify the node exists
        let node = state
            .store
            .get_node(req.from_node_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Node {} not found", req.from_node_id)))?;

        // Verify the node belongs to this session
        if node.session_id != session_id {
            return Err(ApiError::BadRequest(
                "Node does not belong to this session".to_string(),
            ));
        }

        // For creating a branch, we need to set the active_leaf to the parent of the from_node
        // This allows the user to send a new message that becomes a sibling of from_node
        let branch_point_id = node
            .parent_id
            .clone()
            .ok_or_else(|| ApiError::BadRequest("Cannot branch from root node".to_string()))?;

        // Update session to point to the branch point
        // The next message sent will create a new sibling branch
        session.active_leaf_id = branch_point_id.clone();
        session.updated_at = crate::history::node::now();

        // Save updated session
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        crate::logger::log(format!(
            "Branch created from: {}, new active leaf: {}",
            req.from_node_id, branch_point_id
        ));

        Ok(Json(CreateBranchResponse {
            success: true,
            new_leaf_id: branch_point_id.clone(),
            branch_point_id,
            error: None,
        }))
    }

    /// Get all nodes in a session for tree visualization
    pub async fn get_tree(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Get tree request for session: {}", session_id));

        // Verify session exists
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Get all nodes in session using find_nodes with default filter (matches all)
        let filter = crate::history::storage::NodeFilter::default();
        let all_nodes = state
            .store
            .find_nodes(session_id.clone(), filter)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Build children count map
        let mut children_count: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for node in &all_nodes {
            if let Some(ref parent_id) = node.parent_id {
                *children_count.entry(parent_id.clone()).or_insert(0) += 1;
            }
        }

        // Build node map for path traversal
        let node_map: std::collections::HashMap<String, &crate::history::Node> =
            all_nodes.iter().map(|n| (n.node_id.clone(), n)).collect();

        // Calculate context range: traverse from active_leaf to checkpoint or root
        let mut context_node_ids: Vec<String> = Vec::new();
        let mut context_boundary_id: Option<String> = None;
        let mut current_id = Some(session.active_leaf_id.clone());

        while let Some(id) = current_id {
            context_node_ids.push(id.clone());

            // Check if this node has a checkpoint (context boundary)
            if session.checkpoints.contains_key(&id) {
                context_boundary_id = Some(id.clone());
                break;
            }

            // Move to parent
            if let Some(node) = node_map.get(&id) {
                current_id = node.parent_id.clone();
                // Stop at root
                if node.parent_id.is_none() {
                    context_boundary_id = Some(id);
                    break;
                }
            } else {
                break;
            }
        }

        let context_node_set: std::collections::HashSet<String> =
            context_node_ids.iter().cloned().collect();

        // Convert nodes to JSON-serializable format
        let nodes_json: Vec<Value> = all_nodes
            .iter()
            .map(|node| {
                // Check if this node has a checkpoint
                let has_checkpoint = session.checkpoints.contains_key(&node.node_id);
                let checkpoint_data = session.checkpoints.get(&node.node_id);
                // Check if this node is in the current context range
                let in_context = context_node_set.contains(&node.node_id);

                json!({
                    "node_id": node.node_id,
                    "parent_id": node.parent_id,
                    "session_id": node.session_id,
                    "kind": format!("{:?}", node.kind),
                    "role": node.role.as_ref().map(|r| format!("{:?}", r)),
                    "content": &node.content,
                    "tool_calls": &node.tool_calls,
                    "tool_call_id": &node.tool_call_id,
                    "token_usage": &node.token_usage,
                    "created_at": node.created_at,
                    "children_count": children_count.get(&node.node_id).unwrap_or(&0),
                    "has_checkpoint": has_checkpoint,
                    "in_context": in_context,
                    "checkpoint": checkpoint_data.map(|cp| json!({
                        "summary": &cp.summary,
                        "strategy": &cp.strategy,
                        "stats": cp.stats.as_ref().map(|s| json!({
                            "nodes_covered": s.nodes_covered,
                            "total_tokens": s.total_tokens,
                            "summary_tokens": s.summary_tokens,
                            "compression_ratio": s.compression_ratio,
                        })),
                    })),
                })
            })
            .collect();

        Ok(Json(json!({
            "session_id": session_id,
            "root_node_id": session.root_node_id,
            "active_leaf_id": session.active_leaf_id,
            "context_boundary_id": context_boundary_id,
            "context_node_ids": context_node_ids,
            "nodes": nodes_json,
            "total_nodes": all_nodes.len(),
        })))
    }

    /// Get session metrics (active subscribers, event counts, etc.)
    pub async fn get_metrics(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Get metrics request for session: {}", session_id));

        // Get active subscriber count from event bus
        let active_subscribers = state.event_bus.receiver_count();

        // Get total events emitted globally
        let total_events = state.event_bus.current_seq();

        Ok(Json(json!({
            "session_id": session_id,
            "active_subscribers": active_subscribers,
            "total_events_emitted": total_events,
            "timestamp": chrono::Utc::now().timestamp(),
        })))
    }

    /// Export session as Markdown or JSON
    pub async fn export_session(
        Path(session_id): Path<String>,
        Query(params): Query<std::collections::HashMap<String, String>>,
        State(state): State<AppState>,
    ) -> Result<Response, ApiError> {
        use axum::http::header;

        crate::logger::log(format!("Export request for session: {}", session_id));

        // Get export format (default: markdown)
        let format = params
            .get("format")
            .map(|s| s.as_str())
            .unwrap_or("markdown");

        // Load session path
        let path_response = get_path(Path(session_id.clone()), State(state.clone())).await?;
        let path_data: Value =
            serde_json::from_str(&serde_json::to_string(&path_response.0).unwrap()).unwrap();

        let nodes = path_data["nodes"]
            .as_array()
            .ok_or_else(|| ApiError::Internal("Failed to parse nodes".to_string()))?;

        match format {
            "json" => {
                // Return raw JSON
                let content = serde_json::to_string_pretty(&path_data)
                    .map_err(|e| ApiError::Internal(e.to_string()))?;

                Ok(Response::builder()
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"session_{}.json\"", session_id),
                    )
                    .body(content.into())
                    .unwrap())
            }
            "markdown" | _ => {
                // Convert to Markdown
                let mut markdown = String::new();
                markdown.push_str(&format!("# Session: {}\n\n", session_id));
                markdown.push_str(&format!(
                    "Exported: {}\n\n",
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                ));
                markdown.push_str("---\n\n");

                for node in nodes {
                    let role = node["role"].as_str().unwrap_or("Unknown");
                    let content = node["content"].as_str().unwrap_or("");

                    match role {
                        "User" => {
                            markdown.push_str("## 🧑 User\n\n");
                            markdown.push_str(content);
                            markdown.push_str("\n\n");
                        }
                        "Assistant" => {
                            markdown.push_str("## 🤖 Assistant\n\n");
                            markdown.push_str(content);
                            markdown.push_str("\n\n");
                        }
                        "Tool" => {
                            let tool_name = node["tool_call_id"].as_str().unwrap_or("tool");
                            markdown.push_str(&format!("### 🔧 Tool Result ({})\n\n", tool_name));
                            markdown.push_str("```\n");
                            markdown.push_str(content);
                            markdown.push_str("\n```\n\n");
                        }
                        _ => {}
                    }
                }

                Ok(Response::builder()
                    .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
                    .header(
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"session_{}.md\"", session_id),
                    )
                    .body(markdown.into())
                    .unwrap())
            }
        }
    }

    /// Get tree statistics for the session
    pub async fn get_tree_stats(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        crate::logger::log(format!("Get tree stats request for session: {}", session_id));

        // Load session
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        // Get all nodes in session
        let filter = crate::history::storage::NodeFilter::default();
        let all_nodes = state
            .store
            .find_nodes(session_id.clone(), filter)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Build node map for traversal
        let node_map: std::collections::HashMap<String, &crate::history::Node> =
            all_nodes.iter().map(|n| (n.node_id.clone(), n)).collect();

        // === 1. Calculate active path nodes (from last checkpoint/root to active leaf) ===
        let mut active_path_nodes: Vec<&crate::history::Node> = Vec::new();
        let mut last_checkpoint_id: Option<String> = None;
        let mut current_id = Some(session.active_leaf_id.clone());

        while let Some(id) = current_id {
            if let Some(node) = node_map.get(&id) {
                active_path_nodes.push(node);

                // Check if this node has a checkpoint (boundary)
                if session.checkpoints.contains_key(&id) {
                    last_checkpoint_id = Some(id.clone());
                    break;
                }

                // Move to parent
                current_id = node.parent_id.clone();
            } else {
                break;
            }
        }

        // Reverse to get root-to-leaf order
        active_path_nodes.reverse();

        // === 2. Calculate token usage ===
        // Three different token metrics:
        // 1. Path content size: Sum of OUTPUT tokens (new content generated per node)
        // 2. Current context size: INPUT tokens from latest response (full context at that point)
        // 3. Character estimate: Rough approximation (chars / 4)

        let active_path_chars: usize = active_path_nodes
            .iter()
            .map(|n| n.content.len())
            .sum();

        // Sum output tokens (content generated along the path)
        let path_output_tokens: u32 = active_path_nodes
            .iter()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.output_tokens)
            .sum();

        // Get input tokens from the most recent node with token_usage
        // This represents the current context window size
        let current_context_tokens: Option<u32> = active_path_nodes
            .iter()
            .rev()
            .filter_map(|n| n.token_usage.as_ref())
            .map(|u| u.input_tokens)
            .next();

        // Primary metric: Use current context size if available, else path outputs, else estimate
        let active_path_tokens = if let Some(ctx) = current_context_tokens {
            ctx as usize
        } else if path_output_tokens > 0 {
            path_output_tokens as usize
        } else {
            (active_path_chars as f64 / 4.0).ceil() as usize
        };

        // Estimate tokens if we create a checkpoint now
        // Assume checkpoint summary would be ~15% of original (typical compression ratio ~6.7)
        let estimated_summary_tokens = (active_path_tokens as f64 * 0.15).ceil() as usize;
        let potential_savings = active_path_tokens.saturating_sub(estimated_summary_tokens);
        let compression_ratio = if estimated_summary_tokens > 0 {
            active_path_tokens as f64 / estimated_summary_tokens as f64
        } else {
            1.0
        };

        // === 3. Count nodes by type ===
        let mut user_count = 0;
        let mut assistant_count = 0;
        let mut tool_count = 0;
        let mut system_count = 0;
        let checkpoint_count = session.checkpoints.len();

        for node in &all_nodes {
            match node.role {
                Some(crate::history::Role::User) => user_count += 1,
                Some(crate::history::Role::Assistant) => assistant_count += 1,
                Some(crate::history::Role::Tool) => tool_count += 1,
                Some(crate::history::Role::System) => system_count += 1,
                None => {}
            }
        }

        Ok(Json(json!({
            "session_id": session_id,
            "token_usage": {
                "active_path_tokens": active_path_tokens,
                "active_path_chars": active_path_chars,
                "path_output_tokens": path_output_tokens,
                "current_context_tokens": current_context_tokens,
                "char_estimate_tokens": (active_path_chars as f64 / 4.0).ceil() as usize,
                "last_checkpoint_id": last_checkpoint_id,
                "estimated_after_checkpoint": estimated_summary_tokens,
                "potential_token_savings": potential_savings,
                "estimated_compression_ratio": compression_ratio,
            },
            "node_counts": {
                "total": all_nodes.len(),
                "user": user_count,
                "assistant": assistant_count,
                "tool": tool_count,
                "system": system_count,
                "checkpoint": checkpoint_count,
                "active_path": active_path_nodes.len(),
            },
        })))
    }
    // ===== Node Collapse/Expand Operations =====

    pub async fn collapse_node(
        Path((session_id, node_id)): Path<(String, String)>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.collapse_node(node_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Persist
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "success": true,
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }

    pub async fn expand_node(
        Path((session_id, node_id)): Path<(String, String)>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.expand_node(&node_id)
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Persist
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "success": true,
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }

    pub async fn collapse_tool_group(
        Path((session_id, node_id)): Path<(String, String)>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.set_store(state.store.clone());

        session
            .collapse_tool_group(node_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Persist
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "success": true,
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }

    pub async fn get_collapsed_state(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }

    /// Collapse all tool groups in the session (assistant + tool result nodes).
    pub async fn collapse_all_tools(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session.set_store(state.store.clone());

        session
            .auto_collapse_all_tool_groups()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Persist
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "success": true,
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }

    /// Expand all collapsed nodes in the session.
    pub async fn expand_all(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Json<Value>, ApiError> {
        let mut session = state
            .store
            .get_session(session_id.clone())
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or_else(|| ApiError::NotFound(format!("Session {} not found", session_id)))?;

        session
            .expand_all()
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Persist
        state
            .store
            .update_session(&session)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let (collapsed_nodes, collapsed_tool_groups) = session.get_collapse_state();
        Ok(Json(json!({
            "success": true,
            "collapsed_nodes": collapsed_nodes,
            "collapsed_tool_groups": collapsed_tool_groups,
        })))
    }
}

/// Run agent chat in background task
/// Returns the updated session on success
async fn run_agent_chat(
    session: crate::history::Session,
    message: String,
    session_config: SessionConfig,
    config_manager: Arc<ConfigResolver>,
    store: Arc<crate::history::JSONLStore>,
    event_bus: Arc<GlobalEventBus>,
    session_id: String,
    run_id: String,
) -> anyhow::Result<crate::history::Session> {
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
    let mut registry = ToolRegistry::new().register_all_builtin();

    // Add web_search tool if user enabled it in the UI (via enable_grounding config)
    // The tool works with ANY LLM provider (OpenAI, Anthropic, Gemini) - it internally uses Gemini grounding
    if session_config.provider.enable_grounding.unwrap_or(false) {
        if let Ok(web_search_tool) = crate::tools::WebSearchTool::from_env() {
            crate::logger::log(
                "✓ Web search tool loaded (uses Gemini with Google Search grounding internally)"
                    .to_string(),
            );
            registry = registry.register(web_search_tool);
        } else {
            crate::logger::log("⚠️ Web search enabled but GOOGLE_API_KEY not found".to_string());
        }
    }

    // Debug: List all registered tools
    let tool_names = registry.tool_names();
    crate::logger::log(format!(
        "🔧 Registered tools ({}): [{}]",
        tool_names.len(),
        tool_names.join(", ")
    ));

    let mut agent = Agent::new(session, provider, registry);
    agent.set_config(AgentConfig {
        max_rounds: session_config.agent.max_rounds as usize,
        loop_detection: Some(LoopDetectorConfig::default()),
    });

    // Load and inject skills
    let app_home = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".aaagent");
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let skills_manager = crate::skills::SkillsManager::new(&app_home, &cwd);

    let skill_count = skills_manager.skills().len();
    if skill_count > 0 {
        let snapshot = skills_manager.snapshot();
        agent.set_skills_prompt(snapshot.prompt);
        crate::logger::log(format!("🎯 Loaded {} skills", skill_count));
    }

    // Log skill errors if any
    for err in skills_manager.errors() {
        crate::logger::log(format!("⚠️ Skill error: {}", err));
    }

    crate::logger::log(format!(
        "run_agent_chat: Agent initialized (model: {}, max_rounds: {})",
        session_config.provider.model, session_config.agent.max_rounds
    ));

    crate::logger::log(format!(
        "run_agent_chat: Starting chat with message: {}",
        message
    ));

    // Track run-specific sequence number
    let run_seq = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

    // Run chat with callback to stream events
    let result = agent
        .chat_with_callback(&message, |event| {
            let event_bus = event_bus.clone();
            let session_id = session_id.clone();
            let run_id = run_id.clone();
            let run_seq = run_seq.clone();
            async move {
                let seq = run_seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                event_bus.emit(session_id, run_id, seq, event);
            }
        })
        .await;

    match result {
        Ok(_) => {
            crate::logger::log("run_agent_chat: Chat completed successfully".to_string());
            // Return the updated session
            Ok(agent.session)
        }
        Err(e) => {
            // Just return the error - it will be handled by the spawned task
            Err(e)
        }
    }
}

mod sse {
    use super::{ApiError, AppState};
    use crate::history::TreeStore;
    use axum::{
        extract::{Path, State},
        response::sse::{Event, Sse},
    };
    use futures::stream::Stream;
    use std::{convert::Infallible, time::Duration};
    use tokio_stream::wrappers::BroadcastStream;
    use tokio_stream::StreamExt;

    // Intermediate type for SSE events (after async processing)
    struct SseEventData {
        event_type: &'static str,
        data: serde_json::Value,
    }

    pub async fn stream(
        Path((session_id, _stream_id)): Path<(String, String)>,
        State(state): State<AppState>,
    ) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
        crate::logger::log(format!("SSE stream requested for session: {}", session_id));

        // Subscribe to global event bus
        let receiver = state.event_bus.subscribe();

        crate::logger::log(format!(
            "SSE stream connection established for session: {} (active subscribers: {})",
            session_id,
            state.event_bus.receiver_count()
        ));

        // Clone store and session_id for use in async closure
        let store = state.store.clone();
        let session_filter = session_id.clone();

        // Convert broadcast::Receiver to Stream, filter by session_id, then process events
        let event_stream = BroadcastStream::new(receiver)
            .filter_map(move |result| {
                match result {
                    Ok(envelope) => {
                        // Filter events by session_id
                        if envelope.session_id == session_filter {
                            Some(envelope.event)
                        } else {
                            None
                        }
                    }
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(skipped)) => {
                        crate::logger::log(format!(
                            "SSE client lagged, skipped {} events",
                            skipped
                        ));
                        None
                    }
                }
            })
            .then(move |agent_event| {
                let store = store.clone();
                async move {
                    // Convert AgentEvent to SSE Event
                    match agent_event {
                        crate::agent::AgentEvent::Content(content) => SseEventData {
                            event_type: "content",
                            data: serde_json::json!({ "content": content }),
                        },
                        crate::agent::AgentEvent::Thinking(text) => SseEventData {
                            event_type: "thinking",
                            data: serde_json::json!({ "text": text }),
                        },
                        crate::agent::AgentEvent::ToolCallsRequested { tool_calls, content } => SseEventData {
                            event_type: "tool_calls",
                            data: serde_json::json!({
                                "tool_calls": tool_calls,
                                "content": content,
                            }),
                        },
                        crate::agent::AgentEvent::ToolResult {
                            tool_call_id,
                            tool_name,
                            result,
                            is_error,
                        } => SseEventData {
                            event_type: "tool_result",
                            data: serde_json::json!({
                                "tool_call_id": tool_call_id,
                                "tool_name": tool_name,
                                "result": result,
                                "is_error": is_error
                            }),
                        },
                        crate::agent::AgentEvent::LoopDetected { detection } => SseEventData {
                            event_type: "loop_detected",
                            data: serde_json::json!({ "detection": format!("{:?}", detection) }),
                        },
                        crate::agent::AgentEvent::CheckpointCreated { node_id, strategy } => SseEventData {
                            event_type: "checkpoint",
                            data: serde_json::json!({ "node_id": node_id, "strategy": strategy }),
                        },
                        crate::agent::AgentEvent::QueuedMessagesReceived { count } => SseEventData {
                            event_type: "queued_messages",
                            data: serde_json::json!({ "count": count }),
                        },
                        crate::agent::AgentEvent::FollowupProcessed { message_index, total_queued, source } => SseEventData {
                            event_type: "followup_processed",
                            data: serde_json::json!({
                                "message_index": message_index,
                                "total_queued": total_queued,
                                "source": source
                            }),
                        },
                        crate::agent::AgentEvent::Done {
                            total_usage,
                            all_tool_calls,
                            rounds,
                            new_node_ids,
                        } => {
                            // Fetch full node data for each new node ID
                            let mut new_nodes = Vec::new();
                            for node_id in &new_node_ids {
                                if let Ok(Some(node)) = store.get_node(node_id.clone()).await {
                                    new_nodes.push(serde_json::json!({
                                        "node_id": node.node_id,
                                        "parent_id": node.parent_id,
                                        "session_id": node.session_id,
                                        "kind": format!("{:?}", node.kind),
                                        "role": node.role.as_ref().map(|r| format!("{:?}", r)),
                                        "content": &node.content,
                                        "tool_calls": &node.tool_calls,
                                        "tool_call_id": &node.tool_call_id,
                                        "created_at": node.created_at,
                                    }));
                                }
                            }

                            SseEventData {
                                event_type: "done",
                                data: serde_json::json!({
                                    "total_usage": total_usage,
                                    "all_tool_calls": all_tool_calls,
                                    "rounds": rounds,
                                    "new_node_ids": new_node_ids,
                                    "new_nodes": new_nodes
                                }),
                            }
                        }
                        crate::agent::AgentEvent::SubAgentSpawned { run_id, task_label } => SseEventData {
                            event_type: "subagent_spawned",
                            data: serde_json::json!({
                                "run_id": run_id,
                                "task_label": task_label
                            }),
                        },
                        crate::agent::AgentEvent::SubAgentCompleted { run_id, success, error } => SseEventData {
                            event_type: "subagent_completed",
                            data: serde_json::json!({
                                "run_id": run_id,
                                "success": success,
                                "error": error
                            }),
                        },
                    }
                }
            })
            .map(|sse_data| {
                // Create SSE event
                Ok(Event::default()
                    .event(sse_data.event_type)
                    .data(sse_data.data.to_string()))
            });

        // Create SSE response with keepalive
        Ok(Sse::new(event_stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        ))
    }

    /// Stream events for a session (without requiring specific stream_id)
    /// This allows multiple clients to subscribe to a session's events
    pub async fn stream_events(
        Path(session_id): Path<String>,
        State(state): State<AppState>,
    ) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
        crate::logger::log(format!(
            "SSE events stream requested for session: {}",
            session_id
        ));

        // Subscribe to global event bus
        let receiver = state.event_bus.subscribe();

        crate::logger::log(format!(
            "SSE events connection established for session: {} (active subscribers: {})",
            session_id,
            state.event_bus.receiver_count()
        ));

        // Clone store and session_id for use in async closure
        let store = state.store.clone();
        let session_filter = session_id.clone();

        // Convert broadcast::Receiver to Stream, filter by session_id, then process events
        let event_stream = BroadcastStream::new(receiver)
            .filter_map(move |result| {
                match result {
                    Ok(envelope) => {
                        // Filter events by session_id
                        if envelope.session_id == session_filter {
                            Some(envelope.event)
                        } else {
                            None
                        }
                    }
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(
                        skipped,
                    )) => {
                        crate::logger::log(format!(
                            "SSE client lagged, skipped {} events",
                            skipped
                        ));
                        None
                    }
                }
            })
            .then(move |agent_event| {
                let store = store.clone();
                async move {
                    // Convert AgentEvent to SSE Event
                    match agent_event {
                        crate::agent::AgentEvent::Content(content) => SseEventData {
                            event_type: "content",
                            data: serde_json::json!({ "content": content }),
                        },
                        crate::agent::AgentEvent::Thinking(text) => SseEventData {
                            event_type: "thinking",
                            data: serde_json::json!({ "text": text }),
                        },
                        crate::agent::AgentEvent::ToolCallsRequested { tool_calls, content } => {
                            SseEventData {
                                event_type: "tool_calls",
                                data: serde_json::json!({ "tool_calls": tool_calls, "content": content }),
                            }
                        }
                        crate::agent::AgentEvent::ToolResult {
                            tool_call_id,
                            tool_name,
                            result,
                            is_error,
                        } => SseEventData {
                            event_type: "tool_result",
                            data: serde_json::json!({
                                "tool_call_id": tool_call_id,
                                "tool_name": tool_name,
                                "result": result,
                                "is_error": is_error
                            }),
                        },
                        crate::agent::AgentEvent::LoopDetected { detection } => SseEventData {
                            event_type: "loop_detected",
                            data: serde_json::json!({ "detection": format!("{:?}", detection) }),
                        },
                        crate::agent::AgentEvent::CheckpointCreated { node_id, strategy } => {
                            SseEventData {
                                event_type: "checkpoint",
                                data: serde_json::json!({ "node_id": node_id, "strategy": strategy }),
                            }
                        }
                        crate::agent::AgentEvent::QueuedMessagesReceived { count } => SseEventData {
                            event_type: "queued_messages",
                            data: serde_json::json!({ "count": count }),
                        },
                        crate::agent::AgentEvent::FollowupProcessed { message_index, total_queued, source } => SseEventData {
                            event_type: "followup_processed",
                            data: serde_json::json!({
                                "message_index": message_index,
                                "total_queued": total_queued,
                                "source": source
                            }),
                        },
                        crate::agent::AgentEvent::Done {
                            total_usage,
                            all_tool_calls,
                            rounds,
                            new_node_ids,
                        } => {
                            // Fetch full node data for each new node ID
                            let mut new_nodes = Vec::new();
                            for node_id in &new_node_ids {
                                if let Ok(Some(node)) = store.get_node(node_id.clone()).await {
                                    new_nodes.push(serde_json::json!({
                                        "node_id": node.node_id,
                                        "parent_id": node.parent_id,
                                        "session_id": node.session_id,
                                        "kind": format!("{:?}", node.kind),
                                        "role": node.role.as_ref().map(|r| format!("{:?}", r)),
                                        "content": &node.content,
                                        "tool_calls": &node.tool_calls,
                                        "tool_call_id": &node.tool_call_id,
                                        "created_at": node.created_at,
                                    }));
                                }
                            }

                            SseEventData {
                                event_type: "done",
                                data: serde_json::json!({
                                    "total_usage": total_usage,
                                    "all_tool_calls": all_tool_calls,
                                    "rounds": rounds,
                                    "new_node_ids": new_node_ids,
                                    "new_nodes": new_nodes
                                }),
                            }
                        }
                        crate::agent::AgentEvent::SubAgentSpawned { run_id, task_label } => SseEventData {
                            event_type: "subagent_spawned",
                            data: serde_json::json!({
                                "run_id": run_id,
                                "task_label": task_label
                            }),
                        },
                        crate::agent::AgentEvent::SubAgentCompleted { run_id, success, error } => SseEventData {
                            event_type: "subagent_completed",
                            data: serde_json::json!({
                                "run_id": run_id,
                                "success": success,
                                "error": error
                            }),
                        },
                    }
                }
            })
            .map(|sse_data| {
                // Create SSE event
                Ok(Event::default()
                    .event(sse_data.event_type)
                    .data(sse_data.data.to_string()))
            });

        // Create SSE response with keepalive
        Ok(Sse::new(event_stream).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        ))
    }
}
