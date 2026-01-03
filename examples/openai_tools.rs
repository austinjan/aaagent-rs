// Example: Manual tool calling with OpenAI provider (Low-level API)
//
// ⚠️  NOTE: This example uses the manual/low-level API for educational purposes.
// ⚠️  For most use cases, prefer the helper API shown in simple_agent.rs
//
// This example demonstrates:
// - Manual event loop handling
// - Direct control over LoopStep events
// - Custom tool execution logic
//
// Run with: cargo run --example openai_tools
//
// Set OPENAI_API_KEY environment variable before running

use aaagent::llm::{LLMProvider, Message, OpenAIProvider, Role, Tool};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    // Create OpenAI provider
    let provider = OpenAIProvider::create("gpt-5-nano".to_string(), api_key)?;

    println!("🤖 OpenAI Tool Calling Example\n");

    // Define a simple tool using JSON Schema
    let get_weather_tool = Tool {
        name: "get_weather".to_string(),
        description: "Get the current weather for a location".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and country, e.g. San Francisco, CA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "Temperature unit"
                }
            },
            "required": ["location"]
        }),
        full_description: None,
    };

    // Create conversation history
    let messages = vec![Message {
        role: Role::User,
        content: "What's the weather like in San Francisco?".to_string(),
        tool_call_id: None,
        tool_calls: None,
    }];

    println!("💬 User: What's the weather like in San Francisco?");
    println!("🔧 Available tools: get_weather\n");

    // Try to start chat loop with tools
    match provider
        .chat_loop(messages, Some(vec![get_weather_tool]))
        .await
    {
        Ok(mut handle) => {
            println!("✅ chat_loop started successfully!\n");

            // Process events from the chat loop
            while let Some(event_result) = handle.next().await {
                match event_result {
                    Ok(event) => {
                        use aaagent::llm::LoopStep;
                        match event {
                            LoopStep::Thinking(thought) => {
                                // Note: OpenAI models (gpt-4o, gpt-5) don't produce thinking tokens
                                // This is here for compatibility with providers that support it (e.g., Claude)
                                println!("💭 [Thinking: {}]", thought);
                            }
                            LoopStep::Content(text) => {
                                print!("{}", text);
                                std::io::Write::flush(&mut std::io::stdout())?;
                            }
                            LoopStep::ToolCallsRequested {
                                tool_calls,
                                content,
                            } => {
                                if !content.is_empty() {
                                    println!("\n📝 Assistant: {}", content);
                                }
                                println!("\n🔧 Tool calls requested:");
                                for call in &tool_calls {
                                    println!("  - {}: {:?}", call.name, call.arguments);
                                }

                                // Simulate tool execution
                                use aaagent::llm::ToolResult;
                                let results: Vec<ToolResult> = tool_calls
                                    .iter()
                                    .map(|call| ToolResult {
                                        tool_call_id: call.id.clone(),
                                        content:
                                            "The weather in San Francisco is sunny, 72°F (22°C)"
                                                .to_string(),
                                        is_error: false,
                                    })
                                    .collect();

                                println!("\n📤 Submitting tool results...");
                                handle.submit_tool_results(results)?;
                            }
                            LoopStep::ToolResultsReceived { count } => {
                                println!("\n✅ Tool results received (count: {})", count);
                                println!("🤖 Assistant is processing results...\n");
                            }
                            LoopStep::Done {
                                content,
                                total_usage,
                                ..
                            } => {
                                if !content.is_empty() {
                                    println!("\n📝 Final response: {}", content);
                                }
                                println!("\n✅ Conversation complete!");
                                println!("📊 Total usage:");
                                println!("   Input: {} tokens", total_usage.input_tokens);
                                println!("   Output: {} tokens", total_usage.output_tokens);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        println!("\n❌ Error: {:?}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Error: {:?}", e);
        }
    }

    Ok(())
}
