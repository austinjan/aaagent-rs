//! Example demonstrating automatic large output handling in ToolRegistry

use aaagent::llm::{ToolCall, ToolRegistry};
use aaagent::tools::{handle_large_output, ShellTool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Output Handler Integration ===\n");

    // Test the function directly
    println!("1. Testing handle_large_output function:");
    let small = "Small output";
    let result = handle_large_output(small, "test", None)?;
    println!(
        "   Small output: {}",
        if result == small {
            "✓ Passed"
        } else {
            "✗ Failed"
        }
    );

    let large = "X".repeat(3000);
    let result = handle_large_output(&large, "test", None)?;
    println!(
        "   Large output: {}",
        if result.contains("Output too large") {
            "✓ Passed"
        } else {
            "✗ Failed"
        }
    );

    // Test with real tool execution through ToolRegistry
    println!("\n2. Testing ToolRegistry integration:");
    let registry = ToolRegistry::new().register(ShellTool::new());

    // Execute a shell command that produces small output
    let call = ToolCall {
        id: "test1".to_string(),
        name: "shell".to_string(),
        arguments: serde_json::json!({"command": "echo Hello"}),
    };

    if let Some(result) = registry.execute(&call).await {
        println!("   Small command output: {} bytes", result.content.len());
        println!("   Content: {}", result.content.trim());
    }

    // Execute a command that produces large output (list directory recursively)
    let call = ToolCall {
        id: "test2".to_string(),
        name: "shell".to_string(),
        arguments: serde_json::json!({"command": "dir /s"}),
    };

    if let Some(result) = registry.execute(&call).await {
        println!("\n   Large command output: {} bytes", result.content.len());
        if result.content.contains("Output too large") {
            println!("   ✓ Large output handled correctly");
            println!(
                "   Message preview:\n   {}",
                result
                    .content
                    .lines()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n   ")
            );
        } else {
            println!("   ✓ Output was small enough to return directly");
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}
