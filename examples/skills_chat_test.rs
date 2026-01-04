//! Skills Chat Integration Test
//!
//! This example demonstrates testing skills integration with an actual LLM.
//!
//! Run with:
//! ```
//! cargo run --example skills_chat_test --features openai
//! ```

use aaagent::llm::*;
use aaagent::skills::SkillsManager;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    // Create provider
    let provider = OpenAIProvider::create(model.clone(), api_key)?;

    println!("=== Skills Chat Integration Test ===");
    println!("Using model: {}", model);
    println!();

    // Create skills manager using examples directory
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let skills_manager = Arc::new(SkillsManager::new(examples_dir.clone()));

    // Load and display available skills
    let outcome = skills_manager.skills_for_cwd(&examples_dir);
    println!("Available skills:");
    for skill in &outcome.skills {
        println!(
            "  - {} ({}): {}",
            skill.name,
            skill.scope,
            skill.display_description()
        );
    }
    println!();

    // Test 1: Explicit skill reference with /skill:name syntax
    println!("--- Test 1: Explicit skill reference ---");

    let messages = vec![Message {
        role: Role::User,
        content: "Please use /skill:code-review to review this code:\n\n```rust\nfn add(a: i32, b: i32) -> i32 { a + b }\n```".to_string(),
        tool_call_id: None,
        tool_calls: None,
    }];

    let config = ChatLoopConfig::new()
        .with_skills_manager(Arc::clone(&skills_manager))
        .with_cwd(examples_dir.clone())
        .with_auto_parse_skills(true)
        .on_skill_warning(|warning| {
            println!("  Warning: {}", warning);
        })
        .on_content(|text| {
            print!("{}", text);
        })
        .with_max_rounds(5);

    println!("Sending message with explicit skill reference...");
    match chat_loop_with_tools(&provider, messages, vec![], config).await {
        Ok(response) => {
            println!(
                "\n\nResponse received ({} tokens used)",
                response.usage.total()
            );
        }
        Err(e) => {
            println!("\nError: {:?}", e);
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}
