//! Test maintenance module functionality

use aaagent::maintenance::{run_cleanup_tasks, CleanupTask, MaintenanceConfig, TempFileCleanup};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Maintenance Module ===\n");

    // Setup test directory
    let test_dir = PathBuf::from("data/temp");
    fs::create_dir_all(&test_dir)?;

    // Create some test files with different timestamps
    println!("1. Creating test temp files...");
    let old_file = test_dir.join("old_file_test.txt");
    let new_file = test_dir.join("new_file_test.txt");

    fs::write(&old_file, "This is an old file for testing")?;
    println!("   Created: {}", old_file.display());

    // Wait a bit to create time difference
    thread::sleep(Duration::from_secs(2));

    fs::write(&new_file, "This is a new file for testing")?;
    println!("   Created: {}\n", new_file.display());

    // Test cleanup with very short retention (1 second)
    println!("2. Running cleanup with 1 second retention...");
    let config = MaintenanceConfig {
        enabled: true,
        interval_hours: 6,
        tasks: aaagent::maintenance::MaintenanceTasksConfig {
            temp_files: aaagent::maintenance::TempFileCleanupConfig {
                enabled: true,
                retention_hours: 0, // We'll manually set seconds
            },
        },
    };

    // Create custom config with 1 second retention
    let mut test_config = config.clone();
    test_config.tasks.temp_files.retention_hours = 0;

    // Run cleanup that should delete old_file but not new_file
    let task = aaagent::maintenance::TempFileCleanup::new(1); // 1 second retention
    let result = task.run().await;

    match result {
        Ok(count) => println!("   ✓ Cleanup completed: {} files removed\n", count),
        Err(e) => println!("   ✗ Cleanup failed: {}\n", e),
    }

    // Check which files still exist
    println!("3. Checking remaining files:");
    println!("   old_file exists: {}", old_file.exists());
    println!("   new_file exists: {}\n", new_file.exists());

    // Test full run_cleanup_tasks function
    println!("4. Testing run_cleanup_tasks with default config...");
    let default_config = MaintenanceConfig::default();
    let results = run_cleanup_tasks(&default_config).await;

    for (task_name, result) in results {
        match result {
            Ok(count) => println!("   ✓ Task '{}': {} items cleaned", task_name, count),
            Err(e) => println!("   ✗ Task '{}' failed: {}", task_name, e),
        }
    }

    // List remaining temp files
    println!("\n5. Listing all temp files:");
    if let Ok(entries) = fs::read_dir(&test_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                println!("   - {}", entry.file_name().to_string_lossy());
            }
        }
    }

    println!("\n=== Test Complete ===");
    Ok(())
}
