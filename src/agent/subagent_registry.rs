//! SubAgent Registry - Tracks sub-agent runs with persistence
//!
//! Maintains a registry of all sub-agent runs (active and completed) with
//! persistence to disk for recovery after process restarts.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Cleanup strategy for sub-agent sessions after completion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupStrategy {
    /// Delete session immediately after completion
    DeleteImmediately,

    /// Keep session for debugging/inspection
    Keep,
}

impl Default for CleanupStrategy {
    fn default() -> Self {
        Self::Keep
    }
}

/// Outcome of a sub-agent run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubAgentOutcome {
    /// Completed successfully
    Success {
        /// Final output from the sub-agent
        output: String,

        /// Total tokens used
        tokens_used: u32,

        /// Runtime in milliseconds
        runtime_ms: i64,
    },

    /// Failed with error
    Error {
        /// Error message
        error: String,
    },

    /// Timeout exceeded
    Timeout {
        /// Timeout duration in seconds
        timeout_secs: u64,
    },
}

/// A tracked sub-agent run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentRun {
    /// Unique run identifier
    pub run_id: String,

    /// Child session key (unique session for this sub-agent)
    pub child_session_key: String,

    /// Parent session key (main agent that spawned this)
    pub parent_session_key: String,

    /// Human-readable task label
    pub task_label: String,

    /// Cleanup strategy
    pub cleanup: CleanupStrategy,

    /// Unix timestamp when run was created (milliseconds)
    pub created_at: i64,

    /// Unix timestamp when run started (milliseconds)
    pub started_at: Option<i64>,

    /// Unix timestamp when run ended (milliseconds)
    pub ended_at: Option<i64>,

    /// Outcome of the run (if completed)
    pub outcome: Option<SubAgentOutcome>,
}

impl SubAgentRun {
    /// Create a new sub-agent run
    pub fn new(
        run_id: String,
        child_session_key: String,
        parent_session_key: String,
        task_label: String,
        cleanup: CleanupStrategy,
    ) -> Self {
        Self {
            run_id,
            child_session_key,
            parent_session_key,
            task_label,
            cleanup,
            created_at: Utc::now().timestamp_millis(),
            started_at: None,
            ended_at: None,
            outcome: None,
        }
    }

    /// Mark the run as started
    pub fn mark_started(&mut self) {
        self.started_at = Some(Utc::now().timestamp_millis());
    }

    /// Mark the run as completed with outcome
    pub fn mark_completed(&mut self, outcome: SubAgentOutcome) {
        self.ended_at = Some(Utc::now().timestamp_millis());
        self.outcome = Some(outcome);
    }

    /// Check if the run is active (started but not ended)
    pub fn is_active(&self) -> bool {
        self.started_at.is_some() && self.ended_at.is_none()
    }

    /// Check if the run is completed
    pub fn is_completed(&self) -> bool {
        self.ended_at.is_some()
    }

    /// Get elapsed time in milliseconds (from creation to now or end)
    pub fn elapsed_ms(&self) -> i64 {
        let end = self
            .ended_at
            .unwrap_or_else(|| Utc::now().timestamp_millis());
        end - self.created_at
    }
}

/// Registry for tracking sub-agent runs
#[derive(Clone)]
pub struct SubAgentRegistry {
    runs: Arc<Mutex<HashMap<String, SubAgentRun>>>,
    persistence_path: PathBuf,
}

impl SubAgentRegistry {
    /// Create a new registry with persistence to the given path
    pub fn new(persistence_path: PathBuf) -> Self {
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            persistence_path,
        }
    }

    /// Register a new sub-agent run
    pub fn register(&self, run: SubAgentRun) -> Result<()> {
        let mut runs = self.runs.lock().unwrap();

        // Check for duplicate run_id
        if runs.contains_key(&run.run_id) {
            anyhow::bail!("Run ID already exists: {}", run.run_id);
        }

        runs.insert(run.run_id.clone(), run);
        drop(runs); // Release lock before persisting

        self.persist()?;
        Ok(())
    }

    /// Get a run by ID
    pub fn get_run(&self, run_id: &str) -> Option<SubAgentRun> {
        let runs = self.runs.lock().unwrap();
        runs.get(run_id).cloned()
    }

    /// Update a run (e.g., mark as started or completed)
    pub fn update_run(&self, run: SubAgentRun) -> Result<()> {
        let mut runs = self.runs.lock().unwrap();

        if !runs.contains_key(&run.run_id) {
            anyhow::bail!("Run not found: {}", run.run_id);
        }

        runs.insert(run.run_id.clone(), run);
        drop(runs); // Release lock before persisting

        self.persist()?;
        Ok(())
    }

    /// Remove a run from the registry
    pub fn remove_run(&self, run_id: &str) -> Result<()> {
        let mut runs = self.runs.lock().unwrap();

        if runs.remove(run_id).is_none() {
            anyhow::bail!("Run not found: {}", run_id);
        }

        drop(runs); // Release lock before persisting

        self.persist()?;
        Ok(())
    }

    /// Get all active runs
    pub fn get_active_runs(&self) -> Vec<SubAgentRun> {
        let runs = self.runs.lock().unwrap();
        runs.values().filter(|r| r.is_active()).cloned().collect()
    }

    /// Get all runs for a specific parent session
    pub fn get_runs_for_parent(&self, parent_session_key: &str) -> Vec<SubAgentRun> {
        let runs = self.runs.lock().unwrap();
        runs.values()
            .filter(|r| r.parent_session_key == parent_session_key)
            .cloned()
            .collect()
    }

    /// Get count of active runs
    pub fn active_count(&self) -> usize {
        let runs = self.runs.lock().unwrap();
        runs.values().filter(|r| r.is_active()).count()
    }

    /// Persist registry to disk
    fn persist(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.persistence_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let runs = self.runs.lock().unwrap();
        let json = serde_json::to_string_pretty(&*runs)?;

        // Atomic write: write to temp file, then rename
        let temp_path = self.persistence_path.with_extension("tmp");
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, &self.persistence_path)?;

        Ok(())
    }

    /// Restore registry from disk
    pub fn restore(&self) -> Result<()> {
        if !self.persistence_path.exists() {
            log::info!(
                "No registry file found at {:?}, starting fresh",
                self.persistence_path
            );
            return Ok(());
        }

        let json = std::fs::read_to_string(&self.persistence_path)?;
        let restored: HashMap<String, SubAgentRun> = serde_json::from_str(&json)?;

        let mut runs = self.runs.lock().unwrap();
        *runs = restored;

        log::info!("Restored {} sub-agent runs from registry", runs.len());

        Ok(())
    }

    /// Clear all completed runs (for cleanup)
    pub fn clear_completed(&self) -> Result<usize> {
        let mut runs = self.runs.lock().unwrap();

        let before_count = runs.len();
        runs.retain(|_, run| !run.is_completed());
        let removed_count = before_count - runs.len();

        drop(runs); // Release lock before persisting

        if removed_count > 0 {
            self.persist()?;
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_register_and_get_run() {
        let temp = TempDir::new().unwrap();
        let registry_path = temp.path().join("registry.json");
        let registry = SubAgentRegistry::new(registry_path);

        let run = SubAgentRun::new(
            "run-1".to_string(),
            "child-session".to_string(),
            "parent-session".to_string(),
            "Test task".to_string(),
            CleanupStrategy::Keep,
        );

        registry.register(run.clone()).unwrap();

        let retrieved = registry.get_run("run-1").unwrap();
        assert_eq!(retrieved.run_id, "run-1");
        assert_eq!(retrieved.task_label, "Test task");
    }

    #[test]
    fn test_duplicate_run_id_fails() {
        let temp = TempDir::new().unwrap();
        let registry = SubAgentRegistry::new(temp.path().join("registry.json"));

        let run1 = SubAgentRun::new(
            "run-1".to_string(),
            "child-1".to_string(),
            "parent".to_string(),
            "Task 1".to_string(),
            CleanupStrategy::Keep,
        );

        let run2 = SubAgentRun::new(
            "run-1".to_string(), // Same ID
            "child-2".to_string(),
            "parent".to_string(),
            "Task 2".to_string(),
            CleanupStrategy::Keep,
        );

        registry.register(run1).unwrap();
        let result = registry.register(run2);
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_started_and_completed() {
        let temp = TempDir::new().unwrap();
        let registry = SubAgentRegistry::new(temp.path().join("registry.json"));

        let mut run = SubAgentRun::new(
            "run-1".to_string(),
            "child".to_string(),
            "parent".to_string(),
            "Task".to_string(),
            CleanupStrategy::Keep,
        );

        assert!(!run.is_active());
        assert!(!run.is_completed());

        run.mark_started();
        assert!(run.is_active());
        assert!(!run.is_completed());

        run.mark_completed(SubAgentOutcome::Success {
            output: "Done".to_string(),
            tokens_used: 100,
            runtime_ms: 1000,
        });

        assert!(!run.is_active());
        assert!(run.is_completed());
    }

    #[test]
    fn test_persist_and_restore() {
        let temp = TempDir::new().unwrap();
        let registry_path = temp.path().join("registry.json");

        // Create registry and add runs
        {
            let registry = SubAgentRegistry::new(registry_path.clone());

            let run1 = SubAgentRun::new(
                "run-1".to_string(),
                "child-1".to_string(),
                "parent".to_string(),
                "Task 1".to_string(),
                CleanupStrategy::Keep,
            );

            let run2 = SubAgentRun::new(
                "run-2".to_string(),
                "child-2".to_string(),
                "parent".to_string(),
                "Task 2".to_string(),
                CleanupStrategy::DeleteImmediately,
            );

            registry.register(run1).unwrap();
            registry.register(run2).unwrap();
        }

        // Create new registry and restore
        let registry = SubAgentRegistry::new(registry_path);
        registry.restore().unwrap();

        assert!(registry.get_run("run-1").is_some());
        assert!(registry.get_run("run-2").is_some());
    }

    #[test]
    fn test_get_active_runs() {
        let temp = TempDir::new().unwrap();
        let registry = SubAgentRegistry::new(temp.path().join("registry.json"));

        let mut run1 = SubAgentRun::new(
            "run-1".to_string(),
            "child-1".to_string(),
            "parent".to_string(),
            "Task 1".to_string(),
            CleanupStrategy::Keep,
        );
        run1.mark_started();

        let mut run2 = SubAgentRun::new(
            "run-2".to_string(),
            "child-2".to_string(),
            "parent".to_string(),
            "Task 2".to_string(),
            CleanupStrategy::Keep,
        );
        run2.mark_started();
        run2.mark_completed(SubAgentOutcome::Success {
            output: "Done".to_string(),
            tokens_used: 100,
            runtime_ms: 1000,
        });

        registry.register(run1).unwrap();
        registry.register(run2).unwrap();

        let active = registry.get_active_runs();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].run_id, "run-1");
    }

    #[test]
    fn test_clear_completed() {
        let temp = TempDir::new().unwrap();
        let registry = SubAgentRegistry::new(temp.path().join("registry.json"));

        let mut run1 = SubAgentRun::new(
            "run-1".to_string(),
            "child-1".to_string(),
            "parent".to_string(),
            "Task 1".to_string(),
            CleanupStrategy::Keep,
        );
        run1.mark_started();
        run1.mark_completed(SubAgentOutcome::Success {
            output: "Done".to_string(),
            tokens_used: 100,
            runtime_ms: 1000,
        });

        let mut run2 = SubAgentRun::new(
            "run-2".to_string(),
            "child-2".to_string(),
            "parent".to_string(),
            "Task 2".to_string(),
            CleanupStrategy::Keep,
        );
        run2.mark_started();

        registry.register(run1).unwrap();
        registry.register(run2).unwrap();

        let removed = registry.clear_completed().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(registry.active_count(), 1);
    }
}
