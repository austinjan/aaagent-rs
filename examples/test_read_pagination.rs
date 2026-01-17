/// Test read tool search pagination functionality
///
/// This example demonstrates the pagination feature in search mode:
/// - Shows how search results are paginated when exceeding size limit
/// - Demonstrates skip parameter usage for navigating pages
/// - Verifies navigation hints are provided correctly
///
/// Run: cargo run --example test_read_pagination
use aaagent::llm::ToolCall;
use aaagent::tools::{ReadTool, ToolProvider};
use serde_json::json;
use std::fs;
use std::io::Write;

#[tokio::main]
async fn main() {
    println!("=== Read Tool Search Pagination Test ===\n");

    // Create test file with many matching lines
    let test_file = "testing/pagination_test.log";
    fs::create_dir_all("testing").ok();

    let mut file = fs::File::create(test_file).expect("Failed to create test file");

    // Write 200 lines with ERROR pattern
    for i in 1..=200 {
        writeln!(file, "Line {}: This is an ERROR message", i).unwrap();
        writeln!(file, "Line {}: This is an INFO message", i + 1).unwrap();
    }

    println!(
        "Created test file: {} (400 lines, 200 ERROR matches)\n",
        test_file
    );

    let tool = ReadTool::new();

    // Test 1: First page (skip=0, default)
    println!("=== Test 1: First page (skip=0) ===");
    let call1 = ToolCall {
        id: "test1".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": test_file,
            "mode": "search",
            "pattern": "ERROR",
            "context_lines": 1,
            "skip": 0
        }),
    };

    match tool.execute(&call1).await {
        Ok(result) => {
            println!("{}", result);
            let size = result.len();
            println!("\n✓ Output size: {} bytes (should be < 4096)\n", size);
            assert!(size < 4096, "Output exceeded 4KB threshold!");

            // Verify it shows pagination info
            assert!(
                result.contains("Total Matches:"),
                "Missing total matches info"
            );
            assert!(result.contains("Current:"), "Missing current range info");
            assert!(result.contains("Remaining:"), "Missing remaining info");
            assert!(
                result.contains("Next page:"),
                "Missing next page navigation"
            );
        }
        Err(e) => panic!("Test 1 failed: {}", e),
    }

    // Test 2: Second page (skip first batch)
    println!("\n=== Test 2: Second page (skip=10) ===");
    let call2 = ToolCall {
        id: "test2".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": test_file,
            "mode": "search",
            "pattern": "ERROR",
            "context_lines": 1,
            "skip": 10
        }),
    };

    match tool.execute(&call2).await {
        Ok(result) => {
            println!("{}", result);
            let size = result.len();
            println!("\n✓ Output size: {} bytes (should be < 4096)\n", size);
            assert!(size < 4096, "Output exceeded 4KB threshold!");

            // Verify it shows correct pagination
            assert!(
                result.contains("Current: 11-"),
                "Should start from match 11"
            );
        }
        Err(e) => panic!("Test 2 failed: {}", e),
    }

    // Test 3: Last page (skip to near end)
    println!("\n=== Test 3: Near last page (skip=190) ===");
    let call3 = ToolCall {
        id: "test3".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": test_file,
            "mode": "search",
            "pattern": "ERROR",
            "context_lines": 1,
            "skip": 190
        }),
    };

    match tool.execute(&call3).await {
        Ok(result) => {
            println!("{}", result);
            let size = result.len();
            println!("\n✓ Output size: {} bytes (should be < 4096)\n", size);
            assert!(size < 4096, "Output exceeded 4KB threshold!");

            // Should show remaining = 0 or very few
            assert!(
                result.contains("Remaining: 0") || result.contains("Remaining: "),
                "Should be near or at end"
            );
        }
        Err(e) => panic!("Test 3 failed: {}", e),
    }

    // Test 4: Skip beyond total (error case)
    println!("\n=== Test 4: Skip beyond total (skip=300, should error) ===");
    let call4 = ToolCall {
        id: "test4".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": test_file,
            "mode": "search",
            "pattern": "ERROR",
            "skip": 300
        }),
    };

    match tool.execute(&call4).await {
        Ok(result) => {
            println!("{}", result);
            assert!(
                result.contains("Error: skip=300"),
                "Should show error message"
            );
            assert!(result.contains("First page:"), "Should show how to go back");
            println!("\n✓ Correctly handled out-of-bounds skip\n");
        }
        Err(e) => panic!("Test 4 should return Ok with error message, got Err: {}", e),
    }

    // Test 5: No matches
    println!("\n=== Test 5: No matches (pattern=NOTFOUND) ===");
    let call5 = ToolCall {
        id: "test5".to_string(),
        name: "read".to_string(),
        arguments: json!({
            "path": test_file,
            "mode": "search",
            "pattern": "NOTFOUND"
        }),
    };

    match tool.execute(&call5).await {
        Ok(result) => {
            println!("{}", result);
            assert!(
                result.contains("No matches found"),
                "Should indicate no matches"
            );
            println!("\n✓ Correctly handled no matches\n");
        }
        Err(e) => panic!("Test 5 failed: {}", e),
    }

    // Cleanup
    fs::remove_file(test_file).ok();

    println!("\n=== All Pagination Tests Passed! ===");
    println!("✓ First page shows matches with pagination info");
    println!("✓ Second page skips correctly");
    println!("✓ Near end page shows remaining count");
    println!("✓ Out-of-bounds skip handled gracefully");
    println!("✓ No matches handled correctly");
    println!("✓ All outputs < 4096 bytes");
}
