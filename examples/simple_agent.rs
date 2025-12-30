// Example: Simple AI agent using the chat_loop_with_tools helper
//
// This demonstrates:
// - Multi-turn tool calling
// - History tracking with get_history()
// - Tool result pruning (max_tool_turns)
//
// Run with: cargo run --example simple_agent

use km_tools::llm::*;
use km_tools::tools::BashTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");

    let provider = OpenAIProvider::create("gpt-5-nano".to_string(), api_key)?;

    // Configure to keep last 3 tool turns
    provider.update_config(|cfg| {
        cfg.max_tool_turns = Some(3);
    });

    let bash_tool = BashTool::new().with_timeout(30);

    println!("🤖 Simple AI Agent - Multi-turn Tool Calling Test");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let task = "Please help me with the following tasks:\n\
                1. List all children folders in the current directory\n\
                2. Check files in the current folder and give me a summary";

    println!("📝 Tasks:\n{}\n", task);

    // Configure the chat loop with tools and callbacks
    let config = ChatLoopConfig::new()
        // Register the bash tool executor
        .with_tool("bash", {
            let bash_tool = bash_tool.clone();
            move |call| {
                let bash_tool = bash_tool.clone();
                async move { bash_tool.execute(&call).await }
            }
        })
        // Print content as it streams
        .on_content(|text| {
            print!("{}", text);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        })
        // Log when tools are called
        .on_tool_calls(|calls| {
            println!("\n\n🔧 Executing {} tool(s):", calls.len());
            for call in calls {
                if let Some(cmd) = call.arguments.get("command").and_then(|v| v.as_str()) {
                    println!("   $ {}", cmd);
                }
            }
            println!();
        })
        // Set max rounds to prevent infinite loops
        .with_max_rounds(5);

    // Create the initial message
    let messages = vec![Message {
        role: Role::User,
        content: task.to_string(),
        tool_call_id: None,
        tool_calls: None,
    }];

    // Run the chat loop - all handling is automatic!
    let response =
        chat_loop_with_tools(&provider, messages, vec![bash_tool.as_tool()], config).await?;

    // Print summary
    println!("\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Summary:");
    println!("   Rounds: {}", response.rounds);
    println!("   Tools called: {}", response.all_tool_calls.len());
    println!(
        "   Tokens: {} in, {} out (total: {})",
        response.usage.input_tokens,
        response.usage.output_tokens,
        response.usage.total()
    );

    // Get and display conversation history
    println!("\n📜 Conversation History:");
    let history = provider.get_history();
    println!("   Total messages: {}", history.len());

    for (i, msg) in history.iter().enumerate() {
        match msg.role {
            Role::User => {
                println!("\n   [{}] 👤 User:", i + 1);
                println!("      {}", msg.content.lines().next().unwrap_or(""));
                if msg.content.lines().count() > 1 {
                    println!("      ... ({} more lines)", msg.content.lines().count() - 1);
                }
            }
            Role::Assistant => {
                println!("\n   [{}] 🤖 Assistant:", i + 1);
                if let Some(tool_calls) = &msg.tool_calls {
                    println!("      Called {} tool(s):", tool_calls.len());
                    for tc in tool_calls {
                        println!("        - {}", tc.name);
                    }
                } else if !msg.content.is_empty() {
                    println!("      {}", msg.content.lines().next().unwrap_or(""));
                    if msg.content.lines().count() > 1 {
                        println!("      ... ({} more lines)", msg.content.lines().count() - 1);
                    }
                }
            }
            Role::Tool => {
                println!("\n   [{}] 🔧 Tool Result:", i + 1);
                let preview = if msg.content.len() > 100 {
                    format!(
                        "{}... ({} chars total)",
                        &msg.content[..100],
                        msg.content.len()
                    )
                } else {
                    msg.content.clone()
                };
                println!("      {}", preview.lines().next().unwrap_or(""));
                if msg.content.lines().count() > 1 {
                    println!("      ... ({} total lines)", msg.content.lines().count());
                }
            }
            Role::System => {
                println!("\n   [{}] ⚙️  System:", i + 1);
                println!("      {}", msg.content);
            }
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Test Complete!");
    println!("   - Multiple tool calls executed successfully");
    println!("   - History tracked: {} messages", history.len());
    println!(
        "   - Tool pruning configured: max {} turns",
        provider.config().max_tool_turns.unwrap_or(0)
    );
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}
