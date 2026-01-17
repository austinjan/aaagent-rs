//! Maintenance module for periodic cleanup tasks
//!
//! This module provides an extensible framework for running maintenance tasks
//! such as cleaning up old temporary files, rotating logs, etc.

pub mod config;
pub mod tasks;

pub use config::*;
pub use tasks::*;

use tokio::time::{interval, Duration};

/// Run all enabled cleanup tasks once
///
/// Returns a vector of (task_name, result) tuples
pub async fn run_cleanup_tasks(config: &MaintenanceConfig) -> Vec<(String, Result<usize, String>)> {
    let mut results = Vec::new();

    // Temp files cleanup
    if config.tasks.temp_files.enabled {
        let task = TempFileCleanup::new(config.tasks.temp_files.retention_hours * 3600);
        let result = task.run().await;
        results.push((task.name().to_string(), result));
    }

    // Future tasks can be added here
    // Example:
    // if config.tasks.orphaned_streams.enabled {
    //     let task = OrphanedStreamCleanup::new(...);
    //     results.push((task.name().to_string(), task.run().await));
    // }

    results
}

/// Start background maintenance worker
///
/// Spawns a tokio task that runs cleanup tasks at the configured interval.
/// If maintenance is disabled in config, logs a message and returns without spawning.
pub fn start_maintenance_worker(config: MaintenanceConfig) {
    if !config.enabled {
        crate::logger::log("[Maintenance] Worker disabled in config".to_string());
        return;
    }

    tokio::spawn(async move {
        let interval_secs = config.interval_hours * 3600;
        let mut ticker = interval(Duration::from_secs(interval_secs));

        crate::logger::log(format!(
            "[Maintenance] Worker started (interval: {}h)",
            config.interval_hours
        ));

        loop {
            ticker.tick().await;

            crate::logger::log("[Maintenance] Running scheduled cleanup tasks...".to_string());

            let results = run_cleanup_tasks(&config).await;

            for (task_name, result) in results {
                match result {
                    Ok(count) if count > 0 => {
                        crate::logger::log(format!(
                            "[Maintenance] Task '{}' completed: {} items cleaned",
                            task_name, count
                        ));
                    }
                    Ok(_) => {} // Skip logging if 0 items cleaned
                    Err(e) => {
                        crate::logger::log(format!(
                            "[Maintenance] Task '{}' failed: {}",
                            task_name, e
                        ));
                    }
                }
            }
        }
    });
}
