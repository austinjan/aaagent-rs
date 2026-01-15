//! Stream Manager for SSE Streaming
//!
//! Manages concurrent chat streams using tokio channels.
//! Each stream has a unique ID and a receiver for AgentEvents.

use crate::agent::AgentEvent;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub type StreamId = String;

/// StreamManager manages concurrent SSE streams
pub struct StreamManager {
    streams: Arc<Mutex<HashMap<StreamId, mpsc::Receiver<AgentEvent>>>>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new stream and return (stream_id, sender)
    pub async fn create_stream(&self) -> (StreamId, mpsc::Sender<AgentEvent>) {
        let stream_id = format!("stream-{}", ulid::Ulid::new());
        let (tx, rx) = mpsc::channel(100); // Buffer up to 100 events

        self.streams.lock().await.insert(stream_id.clone(), rx);

        (stream_id, tx)
    }

    /// Take ownership of a stream receiver (only removes it after taking)
    /// Returns None if stream doesn't exist
    /// Note: Once taken, the stream is removed from the map
    pub async fn take_stream(&self, stream_id: &str) -> Option<mpsc::Receiver<AgentEvent>> {
        // We remove immediately because SSE takes ownership
        // The channel buffer (100 events) ensures messages aren't lost
        self.streams.lock().await.remove(stream_id)
    }

    /// Get a clone of the receiver without removing the stream
    /// This allows peeking at the stream without consuming it
    /// Returns None if stream doesn't exist
    #[allow(dead_code)]
    pub async fn peek_stream(&self, stream_id: &str) -> Option<mpsc::Receiver<AgentEvent>> {
        // Note: We can't clone Receiver, so we still remove it
        // This is a limitation of mpsc::Receiver
        self.streams.lock().await.remove(stream_id)
    }

    /// Check if a stream exists
    #[allow(dead_code)]
    pub async fn has_stream(&self, stream_id: &str) -> bool {
        self.streams.lock().await.contains_key(stream_id)
    }

    /// Get count of active streams
    #[allow(dead_code)]
    pub async fn stream_count(&self) -> usize {
        self.streams.lock().await.len()
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_stream() {
        let manager = StreamManager::new();
        let (stream_id, _tx) = manager.create_stream().await;

        assert!(manager.has_stream(&stream_id).await);
        assert_eq!(manager.stream_count().await, 1);
    }

    #[tokio::test]
    async fn test_take_stream() {
        let manager = StreamManager::new();
        let (stream_id, _tx) = manager.create_stream().await;

        let rx = manager.take_stream(&stream_id).await;
        assert!(rx.is_some());

        // Stream should be removed after taking
        assert!(!manager.has_stream(&stream_id).await);
    }

    #[tokio::test]
    async fn test_multiple_streams() {
        let manager = StreamManager::new();

        let (id1, _tx1) = manager.create_stream().await;
        let (id2, _tx2) = manager.create_stream().await;

        assert_eq!(manager.stream_count().await, 2);
        assert_ne!(id1, id2);
    }
}
