use aaagent::llm::ToolCall;
use aaagent::tools::{ReadTool, ToolProvider};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("Testing ReadTool...\n");

    let tool = ReadTool::new();

    // Test 1: Read first chunk
    println!("=== Test 1: Read first chunk ===");
    let call1 = ToolCall {
        id: "test1".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": "testing/large_test.txt"
        }),
    };

    match tool.execute(&call1).await {
        Ok(result) => {
            println!("Success! Output length: {} bytes", result.len());
            println!("First 500 chars:\n{}\n", &result[..500.min(result.len())]);
        }
        Err(e) => println!("Error: {}", e),
    }

    // Test 2: Read with offset
    println!("\n=== Test 2: Read from offset ===");
    let call2 = ToolCall {
        id: "test2".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": "testing/large_test.txt",
            "mode": "offset",
            "offset": 3200
        }),
    };

    match tool.execute(&call2).await {
        Ok(result) => {
            println!("Success! Output length: {} bytes", result.len());
            if result.contains("Position: 3200-") {
                println!("✓ Contains correct position info");
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // Test 3: Search
    println!("\n=== Test 3: Search for pattern ===");
    let call3 = ToolCall {
        id: "test3".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": "README.md",
            "mode": "search",
            "pattern": "LLM"
        }),
    };

    match tool.execute(&call3).await {
        Ok(result) => {
            println!("Success! Found matches");
            println!("Output length: {} bytes", result.len());
            if result.contains(":>") {
                println!("✓ Contains match markers");
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // Test 4: Verify output size is under threshold
    println!("\n=== Test 4: Verify all outputs < 4096 bytes ===");
    let test_calls = vec![
        json!({"path": "README.md"}),
        json!({"path": "README.md", "mode": "tail"}),
        json!({"path": "README.md", "mode": "head", "offset": 50}),
    ];

    for (i, args) in test_calls.iter().enumerate() {
        let call = ToolCall {
            id: format!("size_test_{}", i),
            name: "read".to_string(),
            arguments: args.clone(),
        };

        if let Ok(result) = tool.execute(&call).await {
            if result.len() < 4096 {
                println!("✓ Test {}: {} bytes (< 4096)", i + 1, result.len());
            } else {
                println!("✗ Test {}: {} bytes (>= 4096) - FAIL!", i + 1, result.len());
            }
        }
    }

    println!("\n=== All tests complete ===");
}
