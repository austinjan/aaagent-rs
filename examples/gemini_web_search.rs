// Example: Using Gemini with Google Search Grounding
//
// This example demonstrates how to enable web search grounding in Gemini
// to get real-time information from the web.
//
// Usage:
//   cargo run --example gemini_web_search
//
// Prerequisites:
//   - Set GEMINI_API_KEY environment variable
//   - Ensure you have a valid Gemini API key with grounding enabled

use aaagent::llm::{GeminiProvider, LLMProvider};
use futures::StreamExt;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load API key from environment
    let api_key =
        env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY environment variable must be set");

    // Create Gemini provider with gemini-3-flash-preview model
    let provider = GeminiProvider::create("gemini-3-flash-preview".to_string(), api_key)?;

    // Enable web search grounding
    provider.update_config(Box::new(|cfg| {
        cfg.enable_grounding = true;
    }));

    println!("🌐 Gemini Web Search Grounding Example");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Example 1: Ask about current events
    println!("📰 Example 1: Current Events");
    println!("Question: What are the latest developments in AI technology in 2026?");
    println!();

    let mut stream = provider.chat(
        "What are the latest developments in AI technology in 2026? Please provide specific recent news."
    ).await?;

    print!("Response: ");
    while let Some(chunk) = stream.next().await {
        match chunk? {
            aaagent::llm::StreamChunk::Content(text) => {
                print!("{}", text);
            }
            aaagent::llm::StreamChunk::Done {
                finish_reason,
                usage,
                ..
            } => {
                println!("\n");
                println!("✓ Finished (reason: {:?})", finish_reason);
                println!(
                    "📊 Token usage: {} input, {} output",
                    usage.input_tokens, usage.output_tokens
                );
            }
            _ => {}
        }
    }

    // Check for grounding metadata
    let state = provider.state();
    if let Some(metadata) = &state.grounding_metadata {
        println!("\n🔍 Grounding Information:");

        // Extract web search queries
        if let Some(queries) = metadata.get("web_search_queries") {
            if let Some(arr) = queries.as_array() {
                println!("  Search queries executed:");
                for query in arr {
                    if let Some(q) = query.as_str() {
                        println!("    - {}", q);
                    }
                }
            }
        }

        // Extract grounding chunks (web sources)
        if let Some(chunks) = metadata.get("grounding_chunks") {
            if let Some(arr) = chunks.as_array() {
                println!("  Sources:");
                for chunk in arr {
                    if let Some(web) = chunk.get("web") {
                        let uri = web.get("uri").and_then(|u| u.as_str()).unwrap_or("N/A");
                        let title = web
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Untitled");
                        println!("    - {} ({})", title, uri);
                    }
                }
            }
        }
    }

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Example 2: Ask about specific facts
    println!("📊 Example 2: Specific Facts");
    println!("Question: What is the current price of Bitcoin?");
    println!();

    let mut stream = provider
        .chat("What is the current price of Bitcoin in USD?")
        .await?;

    print!("Response: ");
    while let Some(chunk) = stream.next().await {
        match chunk? {
            aaagent::llm::StreamChunk::Content(text) => {
                print!("{}", text);
            }
            aaagent::llm::StreamChunk::Done {
                finish_reason,
                usage,
                ..
            } => {
                println!("\n");
                println!("✓ Finished (reason: {:?})", finish_reason);
                println!(
                    "📊 Token usage: {} input, {} output",
                    usage.input_tokens, usage.output_tokens
                );
            }
            _ => {}
        }
    }

    // Check for grounding metadata again
    let state = provider.state();
    if let Some(metadata) = &state.grounding_metadata {
        println!("\n🔍 Grounding Information:");

        if let Some(queries) = metadata.get("web_search_queries") {
            if let Some(arr) = queries.as_array() {
                println!("  Search queries executed:");
                for query in arr {
                    if let Some(q) = query.as_str() {
                        println!("    - {}", q);
                    }
                }
            }
        }

        if let Some(chunks) = metadata.get("grounding_chunks") {
            if let Some(arr) = chunks.as_array() {
                println!("  Sources ({} total):", arr.len());
                for (i, chunk) in arr.iter().take(5).enumerate() {
                    if let Some(web) = chunk.get("web") {
                        let uri = web.get("uri").and_then(|u| u.as_str()).unwrap_or("N/A");
                        let title = web
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Untitled");
                        println!("    {}. {} ({})", i + 1, title, uri);
                    }
                }
                if arr.len() > 5 {
                    println!("    ... and {} more sources", arr.len() - 5);
                }
            }
        }
    }

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("💡 Note: Grounding with Google Search provides:");
    println!("   - Real-time information from the web");
    println!("   - Verifiable sources and citations");
    println!("   - Reduced hallucinations for factual queries");
    println!("   - Automatic search query generation by the model");

    Ok(())
}
