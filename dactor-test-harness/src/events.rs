use crate::protocol::NodeEvent;
use std::time::Duration;

/// Client-side event stream wrapper for subscribing to node events.
pub struct EventStream {
    inner: tonic::Streaming<NodeEvent>,
}

impl EventStream {
    pub fn new(inner: tonic::Streaming<NodeEvent>) -> Self {
        Self { inner }
    }

    /// Wait for the next event, with timeout.
    pub async fn next_event(&mut self, timeout: Duration) -> Option<NodeEvent> {
        match tokio::time::timeout(timeout, self.inner.message()).await {
            Ok(Ok(Some(event))) => Some(event),
            _ => None,
        }
    }

    /// Wait for an event matching a predicate, with timeout.
    pub async fn expect<F>(&mut self, predicate: F, timeout: Duration) -> Option<NodeEvent>
    where
        F: Fn(&NodeEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.next_event(remaining).await {
                Some(event) if predicate(&event) => return Some(event),
                Some(_) => continue,
                None => return None,
            }
        }
    }
}
