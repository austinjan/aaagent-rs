// Web Search Tool - Search the web using Gemini's built-in Google Search grounding
//
// This tool uses Gemini's native grounding capability to search the web

use crate::llm::{GeminiProvider, LLMProvider, StreamChunk};
use anyhow::{bail, Context, Result};
use futures::StreamExt;
use serde_json::json;

/// Web search tool - uses Gemini's Google Search grounding to search the web
pub struct WebSearchTool {
    gemini_provider: GeminiProvider,
}

impl WebSearchTool {
    /// Create a new web search tool
    ///
    /// Requires Google API key for Gemini with grounding enabled
    pub fn new(gemini_api_key: String) -> Result<Self> {
        if gemini_api_key.trim().is_empty() {
            bail!("Google API key is required for web_search tool. Set GOOGLE_API_KEY environment variable.");
        }

        // Create Gemini provider with grounding enabled
        // Using gemini-3-flash-preview (required for grounding support)
        let gemini_provider =
            GeminiProvider::create("gemini-3-flash-preview".to_string(), gemini_api_key)
                .context("Failed to create Gemini provider for web search")?;

        // Enable grounding for this provider
        gemini_provider.update_config(Box::new(|cfg| {
            cfg.enable_grounding = true;
        }));

        Ok(Self { gemini_provider })
    }

    /// Execute web search with query using Gemini grounding
    pub async fn execute(
        &self,
        query: &str,
        output_handler: Option<Box<dyn Fn(String) + Send>>,
    ) -> Result<String> {
        // Send initial status
        if let Some(ref handler) = output_handler {
            handler(format!("🔍 Searching the web for: \"{}\"\n\n", query));
        }

        // Use Gemini with grounding enabled to search and answer
        // Pass query directly without extra prompting
        let mut stream = self
            .gemini_provider
            .chat(query)
            .await
            .context("Failed to search web with Gemini grounding")?;

        let mut full_response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Content(text)) => {
                    full_response.push_str(&text);
                    // Stream to output handler if provided
                    if let Some(ref handler) = output_handler {
                        handler(text);
                    }
                }
                Ok(StreamChunk::Done { .. }) => {
                    break;
                }
                Err(e) => {
                    let error_msg = format!("Error during Gemini web search: {}", e);
                    // Send error to output handler
                    if let Some(ref handler) = output_handler {
                        handler(format!("❌ {}\n", error_msg));
                    }
                    bail!(error_msg);
                }
                _ => {}
            }
        }

        // Check if we got any response (after stream completes)
        if full_response.trim().is_empty() {
            let msg = format!("Web search returned empty response for query: {}", query);
            if let Some(ref handler) = output_handler {
                handler(format!("⚠️ {}\n", msg));
            }
            bail!(msg);
        }

        // Note: Grounding metadata (sources, citations) is stored in provider state
        // but not directly accessible here. The LLM response should include inline citations.

        Ok(full_response)
    }
}

/// Create web_search tool definition for LLM
pub fn create_tool_definition() -> crate::llm::Tool {
    crate::llm::Tool {
        name: "web_search".to_string(),
        description: "Search the web for current information, news, facts, or recent events using Google Search. Returns comprehensive answers with citations.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "A complete, well-formed question or search query. IMPORTANT: Must be a full question or clear request, NOT just keywords. Examples: 'What is the current spot gold price in USD per ounce?' instead of 'gold price USD'. 'What are the latest developments in AI regulation?' instead of 'AI regulation news'."
                }
            },
            "required": ["query"]
        }),
        full_description: Some(
            r#"Search the web for current information using Gemini's Google Search grounding.

This tool is useful for:
- Recent news and current events
- Real-time data (prices, weather, sports scores, etc.)
- Fact-checking and verification
- Finding sources and references
- Any information beyond the model's knowledge cutoff

The tool uses Gemini's built-in Google Search capability to:
1. Automatically generate relevant search queries
2. Search Google for current information
3. Synthesize results from multiple authoritative sources
4. Provide answers with inline citations

Example usage:
- "What's the latest news about SpaceX Starship?"
- "Current Bitcoin price in USD"
- "Who won the 2026 Super Bowl?"
- "Latest developments in AI regulation"

The response will include citations to sources, making it easy to verify information.
"#
            .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = create_tool_definition();
        assert_eq!(tool.name, "web_search");
        assert!(tool.description.contains("Search the web"));
    }
}
