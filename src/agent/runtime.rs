//! Agent Runtime - Manages agent run lifecycle and message queuing
//!
//! Tracks active agent runs and queues messages for injection when the
//! main agent is busy. Supports FIFO message queuing with configurable
//! depth limits to prevent memory exhaustion.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Maximum number of queued messages per session (default)
pub const DEFAULT_MAX_QUEUE_DEPTH: usize = 100;

/// Maximum time a message can remain in queue (seconds)
pub const DEFAULT_QUEUE_TIMEOUT_SECS: i64 = 300; // 5 minutes

/// Agent runtime that manages active runs and message queuing
#[derive(Clone)]
pub struct AgentRuntime {
    active_runs: Arc<Mutex<HashMap<String, AgentRunHandle>>>,
    message_queues: Arc<Mutex<HashMap<String, Vec<QueuedMessage>>>>,
    max_queue_depth: usize,
    queue_timeout_secs: i64,
}

/// Handle for an active agent run
pub struct AgentRunHandle {
    /// Unique session key for this agent
    pub session_key: String,

    /// Unix timestamp when run started (milliseconds)
    pub started_at: i64,

    /// Whether this run is currently streaming output
    pub is_streaming: bool,

    /// Channel to signal cancellation
    pub cancel_tx: mpsc::Sender<()>,
}

/// A message queued for injection into the agent
#[derive(Clone, Debug)]
pub struct QueuedMessage {
    /// Message content (formatted announcement or user input)
    pub content: String,

    /// Queue processing mode
    pub mode: QueueMode,

    /// Source of the message
    pub source: MessageSource,

    /// Unix timestamp when message was queued (milliseconds)
    pub queued_at: i64,
}

/// Queue processing mode
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueMode {
    /// Process messages sequentially after current turn (Phase 1)
    Followup,

    /// Batch multiple messages into one (Phase 2)
    Collect,

    /// Inject into current turn to guide behavior (Future)
    Steer,

    /// Cancel current turn and process immediately (Future)
    Interrupt,
}

/// Source of an injected message
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageSource {
    /// Message from a completed sub-agent
    SubAgent { run_id: String },

    /// Message from user (multi-client scenario)
    User,

    /// Message from system (e.g., timeout notification)
    System,
}

impl AgentRuntime {
    /// Create a new agent runtime with default settings
    pub fn new() -> Self {
        Self::with_config(DEFAULT_MAX_QUEUE_DEPTH, DEFAULT_QUEUE_TIMEOUT_SECS)
    }

    /// Create a new agent runtime with custom configuration
    pub fn with_config(max_queue_depth: usize, queue_timeout_secs: i64) -> Self {
        Self {
            active_runs: Arc::new(Mutex::new(HashMap::new())),
            message_queues: Arc::new(Mutex::new(HashMap::new())),
            max_queue_depth,
            queue_timeout_secs,
        }
    }

    /// Register an active agent run
    ///
    /// Returns a RunGuard that automatically unregisters the run when dropped.
    pub fn register_run(
        &self,
        session_key: String,
        is_streaming: bool,
    ) -> anyhow::Result<RunGuard> {
        let (cancel_tx, _cancel_rx) = mpsc::channel(1);

        let handle = AgentRunHandle {
            session_key: session_key.clone(),
            started_at: chrono::Utc::now().timestamp_millis(),
            is_streaming,
            cancel_tx,
        };

        let mut runs = self.active_runs.lock().unwrap();

        // Check if already running
        if runs.contains_key(&session_key) {
            anyhow::bail!("Agent run already active for session: {}", session_key);
        }

        runs.insert(session_key.clone(), handle);
        drop(runs); // Release lock

        Ok(RunGuard {
            runtime: self.clone(),
            session_key,
        })
    }

    /// Unregister an active agent run
    fn unregister_run(&self, session_key: &str) {
        let mut runs = self.active_runs.lock().unwrap();
        runs.remove(session_key);
    }

    /// Check if an agent run is currently active for the given session
    pub fn is_run_active(&self, session_key: &str) -> bool {
        let runs = self.active_runs.lock().unwrap();
        runs.contains_key(session_key)
    }

    /// Get information about an active run
    pub fn get_run_info(&self, session_key: &str) -> Option<RunInfo> {
        let runs = self.active_runs.lock().unwrap();
        runs.get(session_key).map(|handle| RunInfo {
            session_key: handle.session_key.clone(),
            started_at: handle.started_at,
            is_streaming: handle.is_streaming,
            elapsed_ms: chrono::Utc::now().timestamp_millis() - handle.started_at,
        })
    }

    /// Enqueue a message for later processing
    ///
    /// Returns `Ok(true)` if message was queued, `Ok(false)` if queue is full,
    /// or an error if the operation failed.
    pub fn enqueue_message(
        &self,
        session_key: String,
        message: QueuedMessage,
    ) -> anyhow::Result<bool> {
        let mut queues = self.message_queues.lock().unwrap();

        let queue = queues.entry(session_key.clone()).or_insert_with(Vec::new);

        // Clean up expired messages first
        self.cleanup_expired_messages_in_queue(queue);

        // Check depth limit
        if queue.len() >= self.max_queue_depth {
            log::warn!(
                "Message queue full for session {} ({}/{})",
                session_key,
                queue.len(),
                self.max_queue_depth
            );
            return Ok(false);
        }

        queue.push(message);

        log::info!(
            "Message queued for session {} ({}/{})",
            session_key,
            queue.len(),
            self.max_queue_depth
        );

        Ok(true)
    }

    /// Drain all queued messages for a session (FIFO order)
    ///
    /// Returns the messages that were queued, oldest first.
    pub fn drain_queue(&self, session_key: &str) -> Vec<QueuedMessage> {
        let mut queues = self.message_queues.lock().unwrap();

        if let Some(queue) = queues.get_mut(session_key) {
            // Clean up expired messages before draining
            self.cleanup_expired_messages_in_queue(queue);

            let messages = std::mem::take(queue);

            if !messages.is_empty() {
                log::info!(
                    "Drained {} messages for session {}",
                    messages.len(),
                    session_key
                );
            }

            messages
        } else {
            Vec::new()
        }
    }

    /// Collect and merge all queued messages for a session (for Collect mode)
    ///
    /// Returns a single merged message string with separators and metadata.
    /// Messages are drained from the queue (removed after collection).
    pub fn collect_messages(&self, session_key: &str) -> Option<String> {
        let messages = self.drain_queue(session_key);

        if messages.is_empty() {
            return None;
        }

        if messages.len() == 1 {
            // Single message - return as-is
            return Some(messages[0].content.clone());
        }

        // Multiple messages - merge with separators
        let mut merged = String::new();
        merged.push_str(&format!(
            "# Batched Updates ({} messages)\n\n",
            messages.len()
        ));

        for (idx, msg) in messages.iter().enumerate() {
            let source_label = match &msg.source {
                MessageSource::SubAgent { run_id } => format!("Sub-Agent: {}", run_id),
                MessageSource::User => "User".to_string(),
                MessageSource::System => "System".to_string(),
            };

            merged.push_str(&format!("## Update {} - {}\n", idx + 1, source_label));

            // Add timestamp if available
            let timestamp = chrono::DateTime::from_timestamp_millis(msg.queued_at);
            if let Some(dt) = timestamp {
                merged.push_str(&format!(
                    "*Queued at: {}*\n\n",
                    dt.format("%Y-%m-%d %H:%M:%S UTC")
                ));
            }

            merged.push_str(&msg.content);
            merged.push_str("\n\n---\n\n");
        }

        log::info!(
            "Collected {} messages into batched update for session {}",
            messages.len(),
            session_key
        );

        Some(merged)
    }

    /// Get current queue depth for a session
    pub fn get_queue_depth(&self, session_key: &str) -> usize {
        let queues = self.message_queues.lock().unwrap();
        queues.get(session_key).map(|q| q.len()).unwrap_or(0)
    }

    /// Get queue metrics for monitoring
    pub fn get_queue_metrics(&self) -> QueueMetrics {
        let queues = self.message_queues.lock().unwrap();
        let runs = self.active_runs.lock().unwrap();

        let total_queued: usize = queues.values().map(|q| q.len()).sum();
        let active_sessions = queues.len();
        let active_runs_count = runs.len();

        QueueMetrics {
            active_runs: active_runs_count,
            active_sessions,
            total_queued_messages: total_queued,
            max_queue_depth: self.max_queue_depth,
        }
    }

    /// Clean up expired messages from a queue
    fn cleanup_expired_messages_in_queue(&self, queue: &mut Vec<QueuedMessage>) {
        let now = chrono::Utc::now().timestamp_millis();
        let timeout_ms = self.queue_timeout_secs * 1000;

        let original_len = queue.len();
        queue.retain(|msg| {
            let age_ms = now - msg.queued_at;
            age_ms < timeout_ms
        });

        let removed = original_len - queue.len();
        if removed > 0 {
            log::warn!("Removed {} expired messages from queue", removed);
        }
    }
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that automatically unregisters a run when dropped
pub struct RunGuard {
    runtime: AgentRuntime,
    session_key: String,
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.runtime.unregister_run(&self.session_key);
        log::debug!("Run guard dropped for session: {}", self.session_key);
    }
}

/// Information about an active run (snapshot)
#[derive(Clone, Debug)]
pub struct RunInfo {
    pub session_key: String,
    pub started_at: i64,
    pub is_streaming: bool,
    pub elapsed_ms: i64,
}

/// Queue metrics for monitoring
#[derive(Clone, Debug)]
pub struct QueueMetrics {
    /// Number of active agent runs
    pub active_runs: usize,

    /// Number of sessions with queued messages
    pub active_sessions: usize,

    /// Total number of queued messages across all sessions
    pub total_queued_messages: usize,

    /// Maximum allowed queue depth per session
    pub max_queue_depth: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_unregister_run() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        // Not active initially
        assert!(!runtime.is_run_active(&session_key));

        // Register run
        let guard = runtime.register_run(session_key.clone(), false).unwrap();
        assert!(runtime.is_run_active(&session_key));

        // Drop guard to unregister
        drop(guard);
        assert!(!runtime.is_run_active(&session_key));
    }

    #[test]
    fn test_cannot_double_register() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        let _guard1 = runtime.register_run(session_key.clone(), false).unwrap();

        // Second registration should fail
        let result = runtime.register_run(session_key.clone(), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_enqueue_and_drain() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        // Enqueue messages
        let msg1 = QueuedMessage {
            content: "Message 1".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        let msg2 = QueuedMessage {
            content: "Message 2".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::User,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        assert!(runtime
            .enqueue_message(session_key.clone(), msg1.clone())
            .unwrap());
        assert!(runtime
            .enqueue_message(session_key.clone(), msg2.clone())
            .unwrap());

        assert_eq!(runtime.get_queue_depth(&session_key), 2);

        // Drain in FIFO order
        let messages = runtime.drain_queue(&session_key);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Message 1");
        assert_eq!(messages[1].content, "Message 2");

        // Queue should be empty now
        assert_eq!(runtime.get_queue_depth(&session_key), 0);
    }

    #[test]
    fn test_queue_depth_limit() {
        let runtime = AgentRuntime::with_config(2, 300); // Max 2 messages
        let session_key = "test-session".to_string();

        let msg = QueuedMessage {
            content: "Test".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        // First two should succeed
        assert!(runtime
            .enqueue_message(session_key.clone(), msg.clone())
            .unwrap());
        assert!(runtime
            .enqueue_message(session_key.clone(), msg.clone())
            .unwrap());

        // Third should fail (queue full)
        let result = runtime
            .enqueue_message(session_key.clone(), msg.clone())
            .unwrap();
        assert!(!result);

        assert_eq!(runtime.get_queue_depth(&session_key), 2);
    }

    #[test]
    fn test_get_run_info() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        let _guard = runtime.register_run(session_key.clone(), true).unwrap();

        let info = runtime.get_run_info(&session_key).unwrap();
        assert_eq!(info.session_key, session_key);
        assert!(info.is_streaming);
        assert!(info.elapsed_ms >= 0);
    }

    #[test]
    fn test_collect_messages_single() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        let msg = QueuedMessage {
            content: "Single message".to_string(),
            mode: QueueMode::Collect,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        runtime.enqueue_message(session_key.clone(), msg).unwrap();

        let collected = runtime.collect_messages(&session_key);
        assert!(collected.is_some());
        assert_eq!(collected.unwrap(), "Single message");
    }

    #[test]
    fn test_collect_messages_multiple() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        let msg1 = QueuedMessage {
            content: "First update".to_string(),
            mode: QueueMode::Collect,
            source: MessageSource::SubAgent {
                run_id: "sub-1".to_string(),
            },
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        let msg2 = QueuedMessage {
            content: "Second update".to_string(),
            mode: QueueMode::Collect,
            source: MessageSource::SubAgent {
                run_id: "sub-2".to_string(),
            },
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        runtime.enqueue_message(session_key.clone(), msg1).unwrap();
        runtime.enqueue_message(session_key.clone(), msg2).unwrap();

        let collected = runtime.collect_messages(&session_key).unwrap();

        // Should be batched with headers
        assert!(collected.contains("# Batched Updates (2 messages)"));
        assert!(collected.contains("## Update 1 - Sub-Agent: sub-1"));
        assert!(collected.contains("## Update 2 - Sub-Agent: sub-2"));
        assert!(collected.contains("First update"));
        assert!(collected.contains("Second update"));
    }

    #[test]
    fn test_collect_messages_empty_queue() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        let collected = runtime.collect_messages(&session_key);
        assert!(collected.is_none());
    }

    #[test]
    fn test_message_expiration() {
        let runtime = AgentRuntime::with_config(100, 0); // 0 second timeout (immediate expiration)
        let session_key = "test-session".to_string();

        // Create message with old timestamp (5 seconds ago)
        let old_msg = QueuedMessage {
            content: "Old message".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis() - 5000,
        };

        runtime
            .enqueue_message(session_key.clone(), old_msg)
            .unwrap();

        // Drain should remove expired messages
        let messages = runtime.drain_queue(&session_key);
        assert_eq!(messages.len(), 0, "Expired messages should be removed");
    }

    #[test]
    fn test_queue_metrics() {
        let runtime = AgentRuntime::new();
        let session1 = "session-1".to_string();
        let session2 = "session-2".to_string();

        // Register a run
        let _guard1 = runtime.register_run(session1.clone(), false).unwrap();

        // Enqueue messages
        let msg = QueuedMessage {
            content: "Test".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        runtime
            .enqueue_message(session1.clone(), msg.clone())
            .unwrap();
        runtime
            .enqueue_message(session1.clone(), msg.clone())
            .unwrap();
        runtime
            .enqueue_message(session2.clone(), msg.clone())
            .unwrap();

        let metrics = runtime.get_queue_metrics();
        assert_eq!(metrics.active_runs, 1);
        assert_eq!(metrics.active_sessions, 2);
        assert_eq!(metrics.total_queued_messages, 3);
        assert_eq!(metrics.max_queue_depth, 100);
    }

    #[test]
    fn test_queue_processing_modes() {
        let runtime = AgentRuntime::new();
        let session_key = "test-session".to_string();

        // Test different queue modes
        let followup_msg = QueuedMessage {
            content: "Followup".to_string(),
            mode: QueueMode::Followup,
            source: MessageSource::User,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        let collect_msg = QueuedMessage {
            content: "Collect".to_string(),
            mode: QueueMode::Collect,
            source: MessageSource::System,
            queued_at: chrono::Utc::now().timestamp_millis(),
        };

        runtime
            .enqueue_message(session_key.clone(), followup_msg.clone())
            .unwrap();
        assert_eq!(runtime.get_queue_depth(&session_key), 1);

        runtime.drain_queue(&session_key);
        assert_eq!(runtime.get_queue_depth(&session_key), 0);

        runtime
            .enqueue_message(session_key.clone(), collect_msg.clone())
            .unwrap();
        assert_eq!(runtime.get_queue_depth(&session_key), 1);
    }
}
