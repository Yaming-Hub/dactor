use std::pin::Pin;

use futures::Stream;

/// A pinned, boxed, Send-safe async stream of items.
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

/// A sender handle given to the actor's stream handler.
/// The actor pushes items into this sender; the caller receives them
/// as an async stream on the other end.
pub struct StreamSender<T: Send + 'static> {
    inner: tokio::sync::mpsc::Sender<T>,
}

impl<T: Send + 'static> StreamSender<T> {
    /// Create a new StreamSender wrapping a tokio mpsc sender.
    pub(crate) fn new(inner: tokio::sync::mpsc::Sender<T>) -> Self {
        Self { inner }
    }

    /// Send an item to the stream consumer.
    /// Returns Err if the consumer has dropped the stream.
    pub async fn send(&self, item: T) -> Result<(), StreamSendError> {
        self.inner
            .send(item)
            .await
            .map_err(|_| StreamSendError::ConsumerDropped)
    }

    /// Try to send without blocking.
    pub fn try_send(&self, item: T) -> Result<(), StreamSendError> {
        self.inner.try_send(item).map_err(|e| match e {
            tokio::sync::mpsc::error::TrySendError::Full(_) => StreamSendError::Full,
            tokio::sync::mpsc::error::TrySendError::Closed(_) => StreamSendError::ConsumerDropped,
        })
    }

    /// Check if the consumer is still listening.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

/// Errors from StreamSender operations.
#[derive(Debug)]
pub enum StreamSendError {
    /// The consumer dropped the stream (no longer reading).
    ConsumerDropped,
    /// The channel buffer is full (backpressure).
    Full,
}

impl std::fmt::Display for StreamSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConsumerDropped => write!(f, "stream consumer dropped"),
            Self::Full => write!(f, "stream buffer full"),
        }
    }
}

impl std::error::Error for StreamSendError {}
