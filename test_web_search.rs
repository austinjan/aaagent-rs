use std::env;

#[tokio::main]
async fn main() {
    // Load .env
    let _ = dotenvy::dotenv();
    
    // Read API key
    let api_key = env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY not set");
    println!("✓ API key loaded: {}...", &api_key.chars().take(10).collect::<String>());
    
    // Create Gemini provider
    use aaagent::llm::{GeminiProvider, LLMProvider};
    let provider = GeminiProvider::create("gemini-3-flash-preview".to_string(), api_key)
        .expect("Failed to create provider");
    
    // Enable grounding
    provider.update_config(Box::new(|cfg| {
        cfg.enable_grounding = true;
    }));
    
    println!("✓ Provider created with grounding enabled");
    
    // Simple query
    let query = "What is the current spot gold price in USD per ounce?";
    println!("\n🔍 Testing query: {}", query);
    
    use futures::StreamExt;
    let mut stream = provider.chat(query).await.expect("Failed to start chat");
    
    let mut response = String::new();
    let mut chunk_count = 0;
    
    while let Some(chunk) = stream.next().await {
        chunk_count += 1;
        match chunk {
            Ok(aaagent::llm::StreamChunk::Content(text)) => {
                print!("{}", text);
                response.push_str(&text);
            }
            Ok(aaagent::llm::StreamChunk::Done { usage, .. }) => {
                println!("\n\n✓ Done! Chunks: {}, Tokens: {:?}", chunk_count, usage);
                break;
            }
            Err(e) => {
                eprintln!("\n✗ Error: {}", e);
                break;
            }
            _ => {}
        }
    }
    
    if response.is_empty() {
        println!("✗ Response is empty!");
    } else {
        println!("✓ Response length: {} chars", response.len());
    }
}
