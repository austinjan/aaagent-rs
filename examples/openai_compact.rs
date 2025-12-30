// Example: Using OpenAI Responses API compact to manage long conversations
//
// This example demonstrates:
// - Building a long conversation history
// - Using the compact() method to reduce token usage
// - Comparing before/after message counts
//
// Run with: cargo run --example openai_compact --features openai
//
// Set OPENAI_API_KEY environment variable before running

use km_tools::llm::{LLMProvider, Message, OpenAIProvider, Role};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    // Create OpenAI provider
    let provider = OpenAIProvider::create("gpt-4o".to_string(), api_key)?;

    println!("🤖 OpenAI Conversation Compaction Example\n");

    // Simulate a long conversation history
    let history = vec![
        Message {
            role: Role::System,
            content: "You are a helpful assistant.".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::User,
            content: "What is the capital of France?".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: "The capital of France is Paris. Paris is not only the capital but also the largest city in France, known for its iconic landmarks like the Eiffel Tower, Louvre Museum, and Notre-Dame Cathedral.".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::User,
            content: "What about Germany?".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: "The capital of Germany is Berlin. Berlin became the capital of reunified Germany in 1990 and is the country's largest city, known for its history, cultural scene, and landmarks like the Brandenburg Gate.".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::User,
            content: "And Italy?".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: "The capital of Italy is Rome. Rome is one of the oldest continuously inhabited cities in the world, famous for historical sites like the Colosseum, Roman Forum, and Vatican City.".to_string(),
            tool_call_id: None,
            tool_calls: None,
        },
    ];

    println!("📊 Original conversation:");
    println!("   Messages: {}", history.len());
    let original_chars: usize = history.iter().map(|m| m.content.len()).sum();
    println!("   Total characters: {}", original_chars);
    println!("   Estimated tokens: ~{}\n", original_chars / 4);

    // Compact the conversation
    println!("🔄 Compacting conversation using OpenAI Responses API...\n");

    match provider.compact(history.clone()).await {
        Ok(compacted) => {
            println!("✅ Compaction successful!\n");
            println!("📊 Compacted conversation:");
            println!("   Messages: {}", compacted.len());
            let compacted_chars: usize = compacted.iter().map(|m| m.content.len()).sum();
            println!("   Total characters: {}", compacted_chars);
            println!("   Estimated tokens: ~{}\n", compacted_chars / 4);

            println!("💾 Savings:");
            let msg_reduction =
                ((history.len() - compacted.len()) as f64 / history.len() as f64) * 100.0;
            let char_reduction =
                ((original_chars - compacted_chars) as f64 / original_chars as f64) * 100.0;
            println!("   Messages reduced: {:.1}%", msg_reduction);
            println!("   Characters reduced: {:.1}%\n", char_reduction);

            println!("📝 Compacted messages:");
            for (i, msg) in compacted.iter().enumerate() {
                println!("   {}. {:?}: {}", i + 1, msg.role, msg.content);
            }
        }
        Err(e) => {
            println!("❌ Compaction failed: {:?}", e);
            println!(
                "\nNote: The Responses API compact endpoint may not be available for all models."
            );
            println!("Try using a newer model like gpt-5.x or check your API access.");
        }
    }

    Ok(())
}
