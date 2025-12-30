// Example: Basic usage of OpenAI provider
//
// Run with: cargo run --example openai_basic --features openai
//
// Set OPENAI_API_KEY environment variable before running

use futures::StreamExt;
use km_tools::llm::{LLMProvider, OpenAIProvider, StreamChunk};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    // Create OpenAI provider
    let provider = OpenAIProvider::create("gpt-5-nano".to_string(), api_key)?;

    println!("🤖 OpenAI Provider Example\n");

    // Configure the provider
    provider.update_config(|config| {
        config.temperature = 1.0;
        config.system_prompt = Some("You are a helpful assistant.".to_string());
    });

    println!("📝 Configuration:");
    let config = provider.config();
    println!("  Temperature: {}", config.temperature);
    println!("  Max tokens: {}", config.max_tokens);
    println!("  System prompt: {:?}\n", config.system_prompt);

    // Stream a chat response
    println!("💬 Question: Explain what Rust is and why it's popular.");
    println!("🔄 Streaming response (watch it appear word by word):\n");

    let mut stream = provider
        .chat("Explain what Rust is and why it's popular in 2-3 sentences.")
        .await?;

    let mut full_response = String::new();
    let mut chunk_count = 0;

    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamChunk::Content(text) => {
                chunk_count += 1;
                print!("{}", text);
                full_response.push_str(&text);
                std::io::Write::flush(&mut std::io::stdout())?;

                // Add small delay to make streaming more visible
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            }
            StreamChunk::Done {
                finish_reason,
                usage,
                ..
            } => {
                println!("\n\n✅ Done!");
                println!("  Finish reason: {:?}", finish_reason);
                println!("  Chunks received: {}", chunk_count);
                println!("  Token usage:");
                println!("    Input: {} tokens", usage.input_tokens);
                println!("    Output: {} tokens", usage.output_tokens);
            }
            _ => {}
        }
    }

    // Check provider state
    println!("\n📊 Provider state:");
    let state = provider.state();
    println!("  Total input tokens: {}", state.input_tokens);
    println!("  Total output tokens: {}", state.output_tokens);
    println!("  Total requests: {}", state.request_count);

    Ok(())
}
