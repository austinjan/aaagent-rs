//! Maintenance configuration types

use serde::{Deserialize, Serialize};

/// Configuration for maintenance tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceConfig {
    /// Enable/disable maintenance worker
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cleanup interval in hours
    #[serde(default = "default_6")]
    pub interval_hours: u64,

    /// Individual task configurations
    #[serde(default)]
    pub tasks: MaintenanceTasksConfig,
}

/// Configuration for all maintenance tasks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceTasksConfig {
    /// Temp file cleanup configuration
    #[serde(default)]
    pub temp_files: TempFileCleanupConfig,
}

/// Configuration for temp file cleanup task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempFileCleanupConfig {
    /// Enable/disable this task
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Retention period in hours
    #[serde(default = "default_6")]
    pub retention_hours: u64,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 6,
            tasks: MaintenanceTasksConfig::default(),
        }
    }
}

impl Default for TempFileCleanupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_hours: 6,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_6() -> u64 {
    6
}
