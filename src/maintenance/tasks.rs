//! Cleanup tasks for maintenance

use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// Trait for cleanup tasks
#[async_trait]
pub trait CleanupTask: Send + Sync {
    /// Task name for logging
    fn name(&self) -> &str;

    /// Execute the cleanup task
    /// Returns: Ok(count) = number of items cleaned, Err = failure reason
    async fn run(&self) -> Result<usize, String>;
}

/// Temp file cleanup task
pub struct TempFileCleanup {
    max_age_secs: u64,
    temp_dir: PathBuf,
}

impl TempFileCleanup {
    /// Create a new temp file cleanup task
    pub fn new(max_age_secs: u64) -> Self {
        Self {
            max_age_secs,
            temp_dir: PathBuf::from("data/temp"),
        }
    }
}

#[async_trait]
impl CleanupTask for TempFileCleanup {
    fn name(&self) -> &str {
        "temp_files"
    }

    async fn run(&self) -> Result<usize, String> {
        let mut count = 0;

        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let now = SystemTime::now();

        let entries =
            fs::read_dir(&self.temp_dir).map_err(|e| format!("Failed to read temp dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let metadata = entry
                .metadata()
                .map_err(|e| format!("Failed to get metadata: {}", e))?;

            if metadata.is_file() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age.as_secs() > self.max_age_secs {
                            match fs::remove_file(entry.path()) {
                                Ok(_) => {
                                    count += 1;
                                    crate::logger::log(format!(
                                        "[Cleanup] Deleted temp file: {} (age: {}s)",
                                        entry.path().display(),
                                        age.as_secs()
                                    ));
                                }
                                Err(e) => {
                                    crate::logger::log(format!(
                                        "[Cleanup] Failed to delete {}: {}",
                                        entry.path().display(),
                                        e
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_temp_file_cleanup() {
        let test_dir = PathBuf::from("testing/maintenance_test");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        // Create new file
        let new_file = test_dir.join("new.txt");
        fs::write(&new_file, "new").unwrap();

        let mut task = TempFileCleanup::new(3600);
        task.temp_dir = test_dir.clone();

        // In real scenario, old files would be deleted
        let result = task.run().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // No files old enough to delete

        // Cleanup
        fs::remove_dir_all(&test_dir).unwrap();
    }
}
