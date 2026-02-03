//! Global Event Bus for broadcasting AgentEvents to multiple SSE clients
//!
//! This module provides a centralized event broadcasting system using tokio::sync::broadcast
//! to support multiple concurrent SSE connections receiving the same events.

use crate::agent::AgentEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::broadcast;

/// Event envelope wrapping AgentEvent with metadata for routing and ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEventEnvelope {
    /// Unique session identifier
    pub session_id: String,

    /// Unique run identifier (agent execution instance)
    pub run_id: String,

    /// Global sequence number (monotonically increasing across all events)
    pub seq: u64,

    /// Run-specific sequence number (resets per run)
    pub run_seq: u64,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// The actual agent event
    pub event: AgentEvent,
}

/// Global event broadcaster supporting multiple subscribers
#[derive(Clone)]
pub struct GlobalEventBus {
    /// Broadcast channel sender (capacity: 1000 events)
    tx: broadcast::Sender<AgentEventEnvelope>,

    /// Global sequence counter (atomic)
    global_seq: Arc<AtomicU64>,
}

impl GlobalEventBus {
    /// Create a new GlobalEventBus with default capacity (1000 events)
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    /// Create a new GlobalEventBus with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);

        Self {
            tx,
            global_seq: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Emit an event to all subscribers
    ///
    /// # Arguments
    /// * `session_id` - Session identifier for filtering
    /// * `run_id` - Run identifier for this agent execution
    /// * `run_seq` - Run-specific sequence number
    /// * `event` - The agent event to broadcast
    ///
    /// # Returns
    /// Number of active subscribers that received the event
    pub fn emit(
        &self,
        session_id: String,
        run_id: String,
        run_seq: u64,
        event: AgentEvent,
    ) -> usize {
        let envelope = AgentEventEnvelope {
            session_id,
            run_id,
            seq: self.global_seq.fetch_add(1, Ordering::SeqCst),
            run_seq,
            timestamp: Utc::now(),
            event,
        };

        // Send returns number of receivers that received the message
        // If all receivers are lagging/dropped, returns 0
        self.tx.send(envelope).unwrap_or(0)
    }

    /// Subscribe to event stream
    ///
    /// # Returns
    /// A broadcast receiver for consuming events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEventEnvelope> {
        self.tx.subscribe()
    }

    /// Get the current number of active subscribers
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Get the current global sequence number (total events emitted)
    pub fn current_seq(&self) -> u64 {
        self.global_seq.load(Ordering::SeqCst)
    }
}

impl Default for GlobalEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::TokenUsage;

    #[tokio::test]
    async fn test_event_bus_basic_emit() {
        let bus = GlobalEventBus::new();
        let mut rx = bus.subscribe();

        let event = AgentEvent::Content("Hello".to_string());
        let count = bus.emit(
            "session-1".to_string(),
            "run-1".to_string(),
            0,
            event.clone(),
        );

        assert_eq!(count, 1, "Should have 1 subscriber");

        let envelope = rx.recv().await.unwrap();
        assert_eq!(envelope.session_id, "session-1");
        assert_eq!(envelope.run_id, "run-1");
        assert_eq!(envelope.seq, 0);
        assert_eq!(envelope.run_seq, 0);
        assert!(matches!(envelope.event, AgentEvent::Content(_)));
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = GlobalEventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.receiver_count(), 2);

        let event = AgentEvent::Thinking("processing...".to_string());
        let count = bus.emit("session-1".to_string(), "run-1".to_string(), 0, event);

        assert_eq!(count, 2, "Should send to 2 subscribers");

        // Both receivers should get the same event
        let envelope1 = rx1.recv().await.unwrap();
        let envelope2 = rx2.recv().await.unwrap();

        assert_eq!(envelope1.seq, envelope2.seq);
        assert_eq!(envelope1.session_id, envelope2.session_id);
    }

    #[tokio::test]
    async fn test_event_bus_sequence_monotonic() {
        let bus = GlobalEventBus::new();
        let mut rx = bus.subscribe();

        // Emit 3 events
        bus.emit(
            "s1".to_string(),
            "r1".to_string(),
            0,
            AgentEvent::Content("1".to_string()),
        );
        bus.emit(
            "s1".to_string(),
            "r1".to_string(),
            1,
            AgentEvent::Content("2".to_string()),
        );
        bus.emit(
            "s1".to_string(),
            "r1".to_string(),
            2,
            AgentEvent::Content("3".to_string()),
        );

        let env1 = rx.recv().await.unwrap();
        let env2 = rx.recv().await.unwrap();
        let env3 = rx.recv().await.unwrap();

        assert_eq!(env1.seq, 0);
        assert_eq!(env2.seq, 1);
        assert_eq!(env3.seq, 2);

        assert_eq!(bus.current_seq(), 3); // Next seq would be 3
    }

    #[tokio::test]
    async fn test_event_bus_session_filtering() {
        let bus = GlobalEventBus::new();
        let mut rx = bus.subscribe();

        // Emit events from different sessions
        bus.emit(
            "session-A".to_string(),
            "run-1".to_string(),
            0,
            AgentEvent::Content("A1".to_string()),
        );
        bus.emit(
            "session-B".to_string(),
            "run-2".to_string(),
            0,
            AgentEvent::Content("B1".to_string()),
        );
        bus.emit(
            "session-A".to_string(),
            "run-1".to_string(),
            1,
            AgentEvent::Content("A2".to_string()),
        );

        // Manually filter for session-A
        let mut session_a_events = Vec::new();
        for _ in 0..3 {
            let envelope = rx.recv().await.unwrap();
            if envelope.session_id == "session-A" {
                session_a_events.push(envelope);
            }
        }

        assert_eq!(session_a_events.len(), 2);
        assert_eq!(session_a_events[0].run_seq, 0);
        assert_eq!(session_a_events[1].run_seq, 1);
    }

    #[tokio::test]
    async fn test_event_bus_all_event_types() {
        let bus = GlobalEventBus::new();
        let mut rx = bus.subscribe();

        let test_events = vec![
            AgentEvent::Content("content".to_string()),
            AgentEvent::Thinking("thinking".to_string()),
            AgentEvent::ToolCallsRequested { tool_calls: vec![] },
            AgentEvent::ToolResult {
                tool_call_id: "tc-1".to_string(),
                tool_name: "test".to_string(),
                result: "ok".to_string(),
                is_error: false,
            },
            AgentEvent::Done {
                total_usage: TokenUsage::default(),
                all_tool_calls: vec![],
                rounds: 1,
                new_node_ids: vec![],
            },
        ];

        for (i, event) in test_events.into_iter().enumerate() {
            bus.emit("s1".to_string(), "r1".to_string(), i as u64, event);
        }

        // Verify all events received
        for _ in 0..5 {
            let envelope = rx.recv().await.unwrap();
            assert_eq!(envelope.session_id, "s1");
        }
    }
}
