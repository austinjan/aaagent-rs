// Example: Interactive AI agent with tree-based history
//
// This demonstrates the new Agent architecture:
// - Session manages conversation tree (with branching support)
// - Provider is stateless (receives Vec<Message> from tree)
// - Tool execution integrated into agent
// - Automatic checkpointing
//
// Run with:
//   cargo run --example interactive_agent_tree --features openai
//   cargo run --example interactive_agent_tree --features "openai gemini" -- --provider=gemini

use aaagent::agent::Agent;
use aaagent::history::{MemoryStore, Session, SessionConfig};
use aaagent::llm::*;
use simplelog::*;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create("app.log")?,
    )?;

    log::debug!("=== Interactive Agent (Tree-based) Starting ===");

    let provider_info = init_provider(parse_provider_kind())
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let ProviderInfo {
        label: provider_label,
        model: provider_model,
        provider,
        quick_provider,
    } = provider_info;
    let provider_name = format!("{} ({})", provider_label, provider_model);
    let has_quick_provider = quick_provider.is_some();

    // Create session with tree storage
    let store = Arc::new(MemoryStore::new());
    let mut config = SessionConfig::default();
    config.system_prompt = Some("You are a helpful AI assistant with access to tools.".to_string());
    config.optimization.checkpoint.every_n_turns = Some(10); // Checkpoint every 10 user turns

    log::info!("═══════════════════════════════════════════════════════════");
    log::info!("Session Configuration:");
    log::info!("  Provider: {}", provider_name);
    log::info!(
        "  Auto checkpoint every: {:?} user turns",
        config.optimization.checkpoint.every_n_turns
    );
    log::info!("═══════════════════════════════════════════════════════════");

    let session = Session::new(store, config).await?;

    // Create registry with all built-in tools
    let registry = ToolRegistry::new().register_all_builtin();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!(
        "║   Interactive AI Agent (Tree-based) - {}    ║",
        provider_name
    );
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Features:");
    println!("  - Tree-based conversation history (supports branching)");
    println!("  - Automatic checkpointing (every 10 user turns)");
    println!("  - Tool execution via ToolRegistry");
    println!("  - Stateless provider (history in tree)");
    if has_quick_provider {
        println!("  - Quick provider enabled for checkpoint summaries");
    }
    println!();
    println!("Commands:");
    println!("  - Type your message to chat");
    println!("  - Type 'exit' or 'quit' to stop");
    println!("  - Type 'branches' to see all conversation branches");
    println!("  - Type 'checkpoints' to see checkpoint info");
    println!();

    // Run the appropriate agent based on provider type
    match provider {
        ActiveProvider::OpenAI(p) => {
            let agent = Agent::new(session, p, registry);
            let mut agent = if let Some(qp) = quick_provider {
                agent.with_quick_provider(qp)
            } else {
                agent
            };
            run_agent_loop(&mut agent).await?;
        }
        #[cfg(feature = "gemini")]
        ActiveProvider::Gemini(p) => {
            let agent = Agent::new(session, p, registry);
            let mut agent = if let Some(qp) = quick_provider {
                agent.with_quick_provider(qp)
            } else {
                agent
            };
            run_agent_loop(&mut agent).await?;
        }
    }

    Ok(())
}

async fn run_agent_loop<P: LLMProvider>(
    agent: &mut Agent<P>,
) -> Result<(), Box<dyn std::error::Error>> {
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

        // Check for EOF or exit commands
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

        // Show branches command
        if user_input.eq_ignore_ascii_case("branches") {
            display_branches(&agent.session).await?;
            continue;
        }

        // Show checkpoints command
        if user_input.eq_ignore_ascii_case("checkpoints") {
            display_checkpoints(&agent.session);
            continue;
        }

        print!("\n🤖 Assistant: ");
        let _ = io::stdout().flush();

        // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
        // Comprehensive Logging for Testing
        // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

        log::info!("════════════════════════════════════════════════════════");
        log::info!("Turn {}: Processing user input", turn);
        log::info!("════════════════════════════════════════════════════════");

        // Get context BEFORE to analyze
        let context = agent.session.get_context().await?;

        // Analyze message composition
        let mut user_msgs = 0;
        let mut assistant_msgs = 0;
        let mut tool_msgs = 0;
        let mut system_msgs = 0;
        let mut total_chars = 0;
        let mut tool_result_sizes = Vec::new();

        for (i, msg) in context.iter().enumerate() {
            let msg_size = msg.content.len();
            total_chars += msg_size;

            match msg.role {
                Role::User => {
                    user_msgs += 1;
                    log::debug!(
                        "  [{}] User ({} chars): {}",
                        i,
                        msg_size,
                        truncate_preview(&msg.content, 50)
                    );
                }
                Role::Assistant => {
                    assistant_msgs += 1;
                    if msg.tool_calls.is_some() {
                        log::debug!("  [{}] Assistant with tool_calls ({} chars)", i, msg_size);
                    } else {
                        log::debug!(
                            "  [{}] Assistant ({} chars): {}",
                            i,
                            msg_size,
                            truncate_preview(&msg.content, 50)
                        );
                    }
                }
                Role::Tool => {
                    tool_msgs += 1;
                    tool_result_sizes.push(msg_size);
                    log::debug!(
                        "  [{}] Tool ({} chars) - tool_call_id: {:?}",
                        i,
                        msg_size,
                        msg.tool_call_id
                    );
                }
                Role::System => {
                    system_msgs += 1;
                    log::debug!("  [{}] System ({} chars)", i, msg_size);
                }
            }
        }

        // Summary statistics
        log::info!("────────────────────────────────────────────────────────");
        log::info!("Context Summary:");
        log::info!("  Total messages: {}", context.len());
        log::info!(
            "  User: {}, Assistant: {}, Tool: {}, System: {}",
            user_msgs,
            assistant_msgs,
            tool_msgs,
            system_msgs
        );
        log::info!("  Total characters: {}", total_chars);

        if !tool_result_sizes.is_empty() {
            let total_tool_chars: usize = tool_result_sizes.iter().sum();
            let avg_tool_size = total_tool_chars / tool_result_sizes.len();
            let max_tool_size = *tool_result_sizes.iter().max().unwrap();
            let min_tool_size = *tool_result_sizes.iter().min().unwrap();

            log::info!("  Tool results: {} messages", tool_result_sizes.len());
            log::info!(
                "    Total tool chars: {} ({:.1}% of context)",
                total_tool_chars,
                (total_tool_chars as f64 / total_chars as f64) * 100.0
            );
            log::info!(
                "    Avg size: {}, Min: {}, Max: {}",
                avg_tool_size,
                min_tool_size,
                max_tool_size
            );
        }

        // Check for checkpoints
        if !agent.session.checkpoints.is_empty() {
            log::info!("  Active checkpoints: {}", agent.session.checkpoints.len());
        }

        log::info!("────────────────────────────────────────────────────────");

        // Display to user
        println!(
            "\n[DEBUG] Context: {} messages, {} chars ({} user, {} assistant, {} tool)",
            context.len(),
            total_chars,
            user_msgs,
            assistant_msgs,
            tool_msgs
        );
        if !tool_result_sizes.is_empty() {
            println!(
                "[DEBUG] Tool results: {} messages, avg {} chars",
                tool_result_sizes.len(),
                tool_result_sizes.iter().sum::<usize>() / tool_result_sizes.len()
            );
        }

        // Track nodes before the chat
        let nodes_before = agent.session.stats.total_nodes;

        // Chat with agent using callback for real-time events
        log::info!("Calling agent.chat_with_callback()...");

        use aaagent::agent::AgentEvent;

        let result = agent
            .chat_with_callback(user_input, |event| async move {
                match event {
                    AgentEvent::Content(text) => {
                        // Stream content as it arrives
                        print!("{}", text);
                        let _ = io::stdout().flush();
                    }
                    AgentEvent::Thinking(thought) => {
                        println!("\n>>> Event: Thinking");
                        log::debug!("💭 Thinking: {}", truncate_preview(&thought, 100));
                        println!("    💭 {}", truncate_preview(&thought, 200));
                    }
                    AgentEvent::ToolCallsRequested { tool_calls } => {
                        println!("\n>>> Event: ToolCallsRequested");
                        println!("    🔧 {} tool(s) requested:", tool_calls.len());
                        for (i, tc) in tool_calls.iter().enumerate() {
                            log::info!("  Tool {}: {} (id: {})", i + 1, tc.name, tc.id);
                            println!("       {}. {} (id: {})", i + 1, tc.name, tc.id);

                            // Pretty print arguments
                            if let Ok(pretty) = serde_json::to_string_pretty(&tc.arguments) {
                                let indented: String = pretty
                                    .lines()
                                    .map(|line| format!("          {}", line))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                println!("{}", indented);
                            }
                        }
                    }
                    AgentEvent::ToolResult {
                        tool_call_id,
                        tool_name,
                        result,
                        is_error,
                    } => {
                        println!("\n>>> Event: ToolResult");
                        log::info!("📦 Tool result: {} (id: {})", tool_name, tool_call_id);

                        let status = if is_error { "❌ ERROR" } else { "✓" };
                        println!("    📦 {} {} (id: {})", status, tool_name, tool_call_id);

                        if result.len() > 500 {
                            println!("       Size: {} chars", result.len());
                            println!("       Preview: {}", truncate_preview(&result, 200));
                        } else {
                            println!("       Result: {}", truncate_preview(&result, 500));
                        }
                    }
                    AgentEvent::LoopDetected { detection } => {
                        println!("\n>>> Event: LoopDetected");
                        log::warn!("🔄 Loop detected: {}", detection.suggestion);
                        println!("    🔄 {}", detection.suggestion);
                        println!("       Action: {:?}", detection.action);
                        if let Some(ref warning) = detection.warning_message {
                            println!("       Warning: {}", warning);
                        }
                    }
                    AgentEvent::CheckpointCreated { node_id, strategy } => {
                        println!("\n>>> Event: CheckpointCreated");
                        log::info!("💾 Checkpoint created: {} ({})", node_id, strategy);
                        println!(
                            "    💾 Created at {} (strategy: {})",
                            truncate_preview(&node_id, 20),
                            strategy
                        );
                    }
                    AgentEvent::Done {
                        total_usage,
                        all_tool_calls,
                        rounds,
                    } => {
                        println!("\n>>> Event: Done");
                        log::info!(
                            "✅ Done: {} rounds, {} tool calls, {} tokens",
                            rounds,
                            all_tool_calls.len(),
                            total_usage.total()
                        );
                        println!("    ✅ Completed");
                        println!("       Rounds: {}", rounds);
                        println!("       Tool calls: {}", all_tool_calls.len());
                        println!(
                            "       Tokens: {} (input: {}, output: {}, cached: {})",
                            total_usage.total(),
                            total_usage.input_tokens,
                            total_usage.output_tokens,
                            total_usage.cached_tokens
                        );
                    }
                }
            })
            .await;

        match result {
            Ok(_response) => {
                // Response already streamed via AgentEvent::Content
                println!(); // Newline after streamed content

                // Show nodes added
                let nodes_after = agent.session.stats.total_nodes;
                let nodes_added = nodes_after - nodes_before;
                log::info!("Turn completed: {} nodes added", nodes_added);
            }
            Err(e) => {
                log::error!("Agent error: {:?}", e);
                println!("\n❌ Error: {:?}", e);
            }
        }

        log::info!("Turn {} completed\n", turn);
    }

    // Show final statistics
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                    Session Summary                         ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    // Get final context to show what would be sent to LLM
    let context = agent.session.get_context().await?;

    // Extract stats after mutable borrow
    let stats = agent.session.stats.clone();

    println!("Total turns: {}", turn - 1);
    println!("Total nodes in tree: {}", stats.total_nodes);
    println!("Active branches: {}", stats.active_branches);
    println!("Total checkpoints: {}", stats.total_checkpoints);
    println!("\nCurrent context size: {} messages", context.len());

    // Detailed final analysis
    log::info!("═══════════════════════════════════════════════════════════");
    log::info!("Final Session Statistics:");
    log::info!("  Total turns: {}", turn - 1);
    log::info!("  Tree nodes: {}", stats.total_nodes);
    log::info!("  Active branches: {}", stats.active_branches);
    log::info!("  Checkpoints created: {}", stats.total_checkpoints);

    // Analyze final context
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut tool_count = 0;
    let mut total_size = 0;

    for msg in &context {
        total_size += msg.content.len();
        match msg.role {
            Role::User => user_count += 1,
            Role::Assistant => assistant_count += 1,
            Role::Tool => tool_count += 1,
            _ => {}
        }
    }

    log::info!("  Final context:");
    log::info!("    - Total messages: {}", context.len());
    log::info!(
        "    - User: {}, Assistant: {}, Tool: {}",
        user_count,
        assistant_count,
        tool_count
    );
    log::info!("    - Total size: {} chars", total_size);

    log::info!("═══════════════════════════════════════════════════════════");
    log::info!("Interactive session ended successfully");

    Ok(())
}

async fn display_branches(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    let branches = session.get_branches().await?;

    println!(
        "\n━━━ Conversation Branches ({} total) ━━━\n",
        branches.len()
    );

    for (i, branch) in branches.iter().enumerate() {
        let marker = if branch.is_active { "→" } else { " " };
        println!(
            "{} {}. Depth: {}, Last updated: {}",
            marker,
            i + 1,
            branch.depth,
            branch.last_updated
        );
        if branch.is_active {
            println!("     (This is the active branch)");
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    Ok(())
}

fn display_checkpoints(session: &Session) {
    println!(
        "\n━━━ Checkpoints ({} total) ━━━\n",
        session.checkpoints.len()
    );

    if session.checkpoints.is_empty() {
        println!("No checkpoints yet.");
    } else {
        for (i, (node_id, checkpoint)) in session.checkpoints.iter().enumerate() {
            println!("{}. Node: {}", i + 1, node_id);
            println!("   Created: {}", checkpoint.created_at);
            if let Some(strategy) = &checkpoint.strategy {
                println!("   Strategy: {}", strategy);
            }
            println!(
                "   Summary: {}...",
                truncate_preview(&checkpoint.summary, 100)
            );
            println!();
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

enum ActiveProvider {
    OpenAI(OpenAIProvider),
    #[cfg(feature = "gemini")]
    Gemini(GeminiProvider),
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
    /// Optional quick provider for simple tasks (checkpoint summaries)
    quick_provider: Option<Box<dyn LLMProvider>>,
}

fn init_provider(kind: ProviderKind) -> Result<ProviderInfo, ProviderError> {
    // Check if quick provider is requested via QUICK_MODEL env var
    let quick_model = env::var("QUICK_MODEL").ok();

    match kind {
        ProviderKind::OpenAI => {
            let api_key =
                env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY environment variable not set");
            let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
            let provider = OpenAIProvider::create(model.clone(), api_key.clone())?;

            // Create quick provider if QUICK_MODEL is set
            let quick_provider: Option<Box<dyn LLMProvider>> = if let Some(ref qm) = quick_model {
                Some(Box::new(OpenAIProvider::create(qm.clone(), api_key)?))
            } else {
                None
            };

            Ok(ProviderInfo {
                label: "OpenAI",
                model,
                provider: ActiveProvider::OpenAI(provider),
                quick_provider,
            })
        }
        ProviderKind::Gemini => {
            #[cfg(feature = "gemini")]
            {
                let api_key = env::var("GEMINI_API_KEY")
                    .expect("GEMINI_API_KEY environment variable not set");
                let model =
                    env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string());
                let provider = GeminiProvider::create(model.clone(), api_key.clone())?;

                // Create quick provider if QUICK_MODEL is set
                let quick_provider: Option<Box<dyn LLMProvider>> = if let Some(ref qm) = quick_model
                {
                    Some(Box::new(GeminiProvider::create(qm.clone(), api_key)?))
                } else {
                    None
                };

                Ok(ProviderInfo {
                    label: "Gemini",
                    model,
                    provider: ActiveProvider::Gemini(provider),
                    quick_provider,
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
            result.push_str(&format!("... ({} chars total)", text.chars().count()));
            return result;
        }
        result.push(ch);
        char_count += 1;
    }
    result
}
