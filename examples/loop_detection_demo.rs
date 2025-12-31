// Example: Loop Detection Demo
//
// This demonstrates the loop detection feature which prevents the LLM
// from getting stuck in repetitive tool calling patterns.
//
// The example simulates different loop scenarios:
// 1. Exact duplicate detection (same tool, same arguments)
// 2. Pattern detection (A→B→A→B oscillation)
//
// Run with: cargo run --example loop_detection_demo --features openai

use km_tools::llm::*;
use km_tools::tools::BashTool;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    let provider = OpenAIProvider::create("gpt-5-nano".to_string(), api_key)?;
    let bash_tool = BashTool::new().with_timeout(10);

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║              Loop Detection Demo (gpt-5-nano)             ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("This demo shows how loop detection prevents infinite loops.");
    println!();

    // Configure loop detection with aggressive settings for demo
    let loop_config = LoopDetectorConfig {
        max_exact_duplicates: 2, // Trigger after 2 duplicates (faster for demo)
        ..Default::default()
    };

    let config = ChatLoopConfig::new()
        .with_tool("bash", {
            let bash_tool = bash_tool.clone();
            move |call| {
                let bash_tool = bash_tool.clone();
                async move { bash_tool.execute(&call).await }
            }
        })
        .on_content(|text| {
            print!("{}", text);
            let _ = io::stdout().flush();
        })
        .on_tool_calls(|calls| {
            println!("\n🔧 Tool calls:");
            for call in calls {
                if let Some(cmd) = call.arguments.get("command").and_then(|v| v.as_str()) {
                    println!("   • {} → {}", call.name, cmd);
                } else {
                    println!("   • {}", call.name);
                }
            }
        })
        .on_loop_detected(|detection| {
            println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("🚨 LOOP DETECTED!");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            println!("Type: {:?}", detection.loop_type);
            println!("Detection count: {}", detection.detection_count);
            println!("Action: {:?}", detection.action);
            println!("Confidence: {:.0}%", detection.confidence * 100.0);
            println!();
            println!("Suggestion:");
            println!("{}", detection.suggestion);
            println!();

            if let Some(ref warning) = detection.warning_message {
                println!("Warning message that would be sent to LLM:");
                println!("─────────────────────────────────────────────────────────────");
                println!("{}", warning);
                println!("─────────────────────────────────────────────────────────────");
                println!();
            }

            // Return based on action
            match detection.action {
                LoopAction::Continue => {
                    println!("→ Continuing (action: Continue)");
                    true
                }
                LoopAction::Warn => {
                    println!("→ Continuing with warning (action: Warn)");
                    true
                }
                LoopAction::Terminate => {
                    println!("→ Terminating (action: Terminate)");
                    false
                }
            }
        })
        .with_loop_detection(loop_config)
        .with_max_rounds(20);

    // Test prompt that might cause a loop
    let test_prompt = "Please check the current directory repeatedly. \
                      Keep checking it even if you already know what's there.";

    println!("📝 Test prompt:");
    println!("   \"{}\"", test_prompt);
    println!();
    println!("🤖 LLM Response:");
    println!();

    let messages = vec![Message {
        role: Role::User,
        content: test_prompt.to_string(),
        tool_call_id: None,
        tool_calls: None,
    }];

    match chat_loop_with_tools(&provider, messages, vec![bash_tool.as_tool()], config).await {
        Ok(response) => {
            println!();
            println!();
            println!("╔════════════════════════════════════════════════════════════╗");
            println!("║                     Completed Successfully                 ║");
            println!("╚════════════════════════════════════════════════════════════╝");
            println!();
            println!("Rounds: {}", response.rounds);
            println!("Total tool calls: {}", response.all_tool_calls.len());
            println!("Token usage:");
            println!("  - Input:  {}", response.usage.input_tokens);
            println!("  - Output: {}", response.usage.output_tokens);
            println!("  - Total:  {}", response.usage.total());
        }
        Err(e) => {
            println!();
            println!();
            println!("╔════════════════════════════════════════════════════════════╗");
            println!("║                  Terminated Due to Loop                    ║");
            println!("╚════════════════════════════════════════════════════════════╝");
            println!();
            println!("Error: {:?}", e);
            println!();
            println!("This is the expected behavior when loop detection prevents");
            println!("an infinite loop!");
        }
    }

    Ok(())
}
