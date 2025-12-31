// Example: Interactive AI agent with detailed tool call logging
//
// This demonstrates:
// - Interactive conversation loop
// - Detailed tool call/result logging
// - History tracking across multiple turns
// - User can continue or exit
//
// Run with:
//   cargo run --example interactive_agent --features openai
//   cargo run --example interactive_agent --features "openai gemini" -- --provider=gemini

use km_tools::llm::*;
use simplelog::*;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger to write to app.log
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create("app.log")?,
    )?;

    log::debug!("=== Interactive Agent Starting ===");

    let provider_info = init_provider(parse_provider_kind())
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let ProviderInfo {
        label: provider_label,
        model: provider_model,
        provider,
    } = provider_info;
    let provider_name = format!("{} ({})", provider_label, provider_model);
    let provider = provider;

    // Configure to keep last 5 tool turns
    provider.update_config(|cfg| {
        cfg.max_tool_turns = Some(5);
    });

    // Create registry with all built-in tools
    // Wrapped in Arc<RwLock<>> to share mutable state across turns
    let registry = Arc::new(RwLock::new(ToolRegistry::new().register_all_builtin()));

    println!("╔════════════════════════════════════════════════════════════╗");
    println!(
        "║     Interactive AI Agent with Tool Registry ({})  ║",
        provider_name
    );
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Features:");
    println!("  - Dynamic tool loading via ToolRegistry");
    println!("  - LLM can pick tools as needed with pick_tools");
    println!("  - Detailed tool call/result logging");
    println!("  - History tracking across turns");
    println!("  - Type 'exit' or 'quit' to stop");
    println!("  - Type 'history' to see conversation history");
    println!();

    let mut conversation_history = Vec::new();
    let mut turn = 0;

    loop {
        turn += 1;

        // Get user input
        print!("\n──── Turn {} ────\n", turn);
        print!("👤 You: ");
        io::stdout().flush()?;

        let mut user_input = String::new();
        let bytes_read = io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        // Check for EOF (piped input ended) or exit commands
        if bytes_read == 0 {
            println!("\n👋 Goodbye!");
            break;
        }
        if user_input.is_empty() {
            continue;
        }
        if user_input.eq_ignore_ascii_case("exit") || user_input.eq_ignore_ascii_case("quit") {
            println!("\n👋 Goodbye!");
            break;
        }

        // Show history command
        if user_input.eq_ignore_ascii_case("history") {
            display_history(&provider);
            continue;
        }

        // Add user message to history
        conversation_history.push(Message {
            role: Role::User,
            content: user_input.to_string(),
            tool_call_id: None,
            tool_calls: None,
        });

        print!("\n🤖 Assistant: ");
        let _ = io::stdout().flush();

        // Configure the chat loop with registry and detailed logging
        let config = ChatLoopConfig::new()
            .with_registry(Arc::clone(&registry))
            .on_content(|text| {
                // Print each chunk as it arrives for visible streaming effect
                print!("{}", text);
                let _ = io::stdout().flush();
            })
            .on_tool_calls(|calls| {
                println!(
                    "\n🔧 Calling {} tool{}:",
                    calls.len(),
                    if calls.len() == 1 { "" } else { "s" }
                );
                for (i, call) in calls.iter().enumerate() {
                    if let Some(cmd) = call.arguments.get("command").and_then(|v| v.as_str()) {
                        println!("   {}. {} → {}", i + 1, call.name, cmd);
                    } else {
                        println!("   {}. {}", i + 1, call.name);
                    }
                }
                println!("⏳ Executing...\n");
            })
            .on_tool_results(|results| {
                println!(
                    "✅ Result{} received:",
                    if results.len() == 1 { "" } else { "s" }
                );
                for (i, result) in results.iter().enumerate() {
                    let preview = truncate_preview(&result.content, 200);

                    if result.is_error {
                        println!("   {}. ❌ Error:", i + 1);
                    } else {
                        println!("   {}. ✓ Success:", i + 1);
                    }

                    for (j, line) in preview.lines().enumerate() {
                        if j < 5 {
                            println!("      {}", line);
                        } else {
                            println!("      ... ({} more lines)", preview.lines().count() - 5);
                            break;
                        }
                    }
                }
                println!();
            })
            .with_max_rounds(30);

        // Run the chat loop
        // Get initial tools from registry (includes pick_tools meta-tool)
        let tools = {
            let reg = registry.read().unwrap();
            reg.get_tools_for_llm()
        };
        match provider
            .run_chat_loop(conversation_history.clone(), tools, config)
            .await
        {
            Ok(_response) => {
                // Update conversation history from provider
                conversation_history = provider.get_history();
                // Tool results are already shown inline during execution
            }
            Err(e) => {
                println!("\n❌ Error: {:?}", e);
            }
        }
    }

    // Show final statistics
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                    Session Summary                         ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let state = provider.state();
    println!("Total turns: {}", turn - 1);
    println!("Total messages in history: {}", conversation_history.len());
    println!("Total tokens used:");
    println!("  - Input:  {}", state.input_tokens);
    println!("  - Output: {}", state.output_tokens);
    println!("  - Total:  {}", state.input_tokens + state.output_tokens);
    println!("API requests: {}", state.request_count);

    Ok(())
}

fn display_history(provider: &ActiveProvider) {
    let history = provider.get_history();

    println!(
        "\n━━━ Conversation History ({} messages) ━━━\n",
        history.len()
    );

    for (i, msg) in history.iter().enumerate() {
        match msg.role {
            Role::User => {
                println!("{}. 👤 You:", i + 1);
                for line in msg.content.lines() {
                    println!("   {}", line);
                }
                println!();
            }
            Role::Assistant => {
                println!("{}. 🤖 Assistant:", i + 1);
                if let Some(tool_calls) = &msg.tool_calls {
                    println!("   Called {} tool(s):", tool_calls.len());
                    for tc in tool_calls {
                        if let Some(cmd) = tc.arguments.get("command").and_then(|v| v.as_str()) {
                            println!("   • {} → {}", tc.name, cmd);
                        } else {
                            println!("   • {}", tc.name);
                        }
                    }
                }
                if !msg.content.is_empty() {
                    for (j, line) in msg.content.lines().enumerate() {
                        if j < 3 {
                            println!("   {}", line);
                        } else if j == 3 {
                            println!("   ... ({} more lines)", msg.content.lines().count() - 3);
                            break;
                        }
                    }
                }
                println!();
            }
            Role::Tool => {
                println!("{}. 🔧 Result:", i + 1);
                let preview = truncate_preview(&msg.content, 150);
                for (j, line) in preview.lines().enumerate() {
                    if j < 3 {
                        println!("   {}", line);
                    } else if j == 3 {
                        println!(
                            "   ... ({} lines, {} chars total)",
                            msg.content.lines().count(),
                            msg.content.len()
                        );
                        break;
                    }
                }
                println!();
            }
            Role::System => {
                println!("{}. ⚙️  System:", i + 1);
                println!("   {}\n", msg.content);
            }
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

enum ActiveProvider {
    OpenAI(OpenAIProvider),
    #[cfg(feature = "gemini")]
    Gemini(GeminiProvider),
}

impl ActiveProvider {
    fn update_config(&self, f: impl FnOnce(&mut ProviderConfig)) {
        match self {
            ActiveProvider::OpenAI(p) => p.update_config(f),
            #[cfg(feature = "gemini")]
            ActiveProvider::Gemini(p) => p.update_config(f),
        }
    }

    fn get_history(&self) -> Vec<Message> {
        match self {
            ActiveProvider::OpenAI(p) => p.get_history(),
            #[cfg(feature = "gemini")]
            ActiveProvider::Gemini(p) => p.get_history(),
        }
    }

    fn state(&self) -> ProviderState {
        match self {
            ActiveProvider::OpenAI(p) => p.state(),
            #[cfg(feature = "gemini")]
            ActiveProvider::Gemini(p) => p.state(),
        }
    }

    async fn run_chat_loop(
        &self,
        history: Vec<Message>,
        tools: Vec<Tool>,
        config: ChatLoopConfig,
    ) -> Result<ChatLoopResponse, ProviderError> {
        match self {
            ActiveProvider::OpenAI(p) => chat_loop_with_tools(p, history, tools, config).await,
            #[cfg(feature = "gemini")]
            ActiveProvider::Gemini(p) => chat_loop_with_tools(p, history, tools, config).await,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    OpenAI,
    Gemini,
}

fn parse_provider_kind() -> ProviderKind {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--provider=") {
            return provider_kind_from_str(value);
        } else if arg == "--provider" {
            if let Some(value) = args.next() {
                return provider_kind_from_str(&value);
            }
        }
    }
    ProviderKind::OpenAI
}

fn provider_kind_from_str(value: &str) -> ProviderKind {
    match value.to_lowercase().as_str() {
        "gemini" => ProviderKind::Gemini,
        "openai" => ProviderKind::OpenAI,
        _ => ProviderKind::OpenAI,
    }
}

struct ProviderInfo {
    label: &'static str,
    model: String,
    provider: ActiveProvider,
}

fn init_provider(kind: ProviderKind) -> Result<ProviderInfo, ProviderError> {
    match kind {
        ProviderKind::OpenAI => {
            let api_key =
                env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
            let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5-nano".to_string());
            let provider = OpenAIProvider::create(model.clone(), api_key)?;
            Ok(ProviderInfo {
                label: "OpenAI",
                model,
                provider: ActiveProvider::OpenAI(provider),
            })
        }
        ProviderKind::Gemini => {
            #[cfg(feature = "gemini")]
            {
                let api_key = env::var("GEMINI_API_KEY")
                    .expect("GEMINI_API_KEY environment variable not set");
                let model = env::var("GEMINI_MODEL")
                    .unwrap_or_else(|_| "gemini-3-flash-preview".to_string());
                let provider = GeminiProvider::create(model.clone(), api_key)?;
                Ok(ProviderInfo {
                    label: "Gemini",
                    model,
                    provider: ActiveProvider::Gemini(provider),
                })
            }
            #[cfg(not(feature = "gemini"))]
            {
                Err(ProviderError::ConfigError(
                    "Gemini provider requested but the 'gemini' feature is not enabled."
                        .to_string(),
                ))
            }
        }
    }
}

fn truncate_preview(text: &str, limit: usize) -> String {
    let mut result = String::new();
    let mut char_count = 0;
    for ch in text.chars() {
        if char_count >= limit {
            result.push_str(&format!("... ({} chars)", text.chars().count()));
            return result;
        }
        result.push(ch);
        char_count += 1;
    }
    result
}
