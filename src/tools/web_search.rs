// Web Search Tool Provider - ToolProvider wrapper for web_search functionality

use crate::llm::tools::web_search::{create_tool_definition, WebSearchTool as WebSearchCore};
use crate::llm::ToolCall;
use crate::tools::{BoxFuture, ToolProvider};
use serde_json::json;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

// Cache GOOGLE_API_KEY by reading .env file directly (avoids env var issues in async contexts)
static CACHED_GOOGLE_API_KEY: OnceLock<Option<String>> = OnceLock::new();

/// Read GOOGLE_API_KEY from .env file directly
fn read_google_api_key_from_env_file() -> Option<String> {
    // Try to read .env file from current directory
    let env_path = std::path::Path::new(".env");
    if !env_path.exists() {
        return None;
    }

    // Read and parse .env file
    if let Ok(content) = std::fs::read_to_string(env_path) {
        for line in content.lines() {
            let line = line.trim();
            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            // Look for GOOGLE_API_KEY=value
            if let Some(value) = line.strip_prefix("GOOGLE_API_KEY=") {
                let value = value.trim();
                // Remove quotes if present
                let value = value.trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Web Search tool provider
pub struct WebSearchTool {
    core: Arc<Mutex<WebSearchCore>>,
}

impl WebSearchTool {
    /// Create a new web search tool
    ///
    /// Requires Google API key (for Gemini with grounding)
    pub fn new(gemini_api_key: String) -> Result<Self, String> {
        let core = WebSearchCore::new(gemini_api_key)
            .map_err(|e| format!("Failed to create web search tool: {}", e))?;

        Ok(Self {
            core: Arc::new(Mutex::new(core)),
        })
    }

    /// Try to create by reading GOOGLE_API_KEY from .env file
    ///
    /// Reads .env file directly (cached on first access to avoid async context issues)
    pub fn from_env() -> Result<Self, String> {
        // Get cached key or initialize cache by reading .env file
        let cached_key = CACHED_GOOGLE_API_KEY.get_or_init(read_google_api_key_from_env_file);

        let google_key = cached_key.as_ref().ok_or_else(|| {
            "GOOGLE_API_KEY not found in .env file (required for web search)".to_string()
        })?;

        Self::new(google_key.clone())
    }
}

impl ToolProvider for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn brief(&self) -> &str {
        "Search the web for current information, news, facts, or recent events using Google Search"
    }

    fn full_description(&self) -> String {
        let tool_def = create_tool_definition();
        tool_def.get_full_description().to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "A complete, well-formed question or search query. IMPORTANT: Must be a full question or clear request, NOT just keywords. Examples: 'What is the current spot gold price in USD per ounce?' instead of 'gold price USD'. 'What are the latest developments in AI regulation?' instead of 'AI regulation news'."
                }
            },
            "required": ["query"]
        })
    }

    fn execute<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            // Parse arguments
            let query = call
                .arguments
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter: query".to_string())?;

            // Create output handler that logs to console
            // TODO: Hook this up to agent's event stream
            let output_handler: Option<Box<dyn Fn(String) + Send>> = Some(Box::new(|text| {
                print!("{}", text);
            }));

            // Execute search
            let core = self.core.lock().await;
            let result = core.execute(query, output_handler).await;

            result.map_err(|e| format!("Web search failed: {}", e))
        })
    }
}
