/// Mailbox capacity configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxConfig {
    /// Unbounded mailbox — no capacity limit (default).
    Unbounded,
    /// Bounded mailbox with a fixed capacity.
    Bounded {
        capacity: usize,
        overflow: OverflowStrategy,
    },
}

impl Default for MailboxConfig {
    fn default() -> Self {
        Self::Unbounded
    }
}

/// What happens when a bounded mailbox is full.
///
/// `DropOldest` is intentionally omitted — no provider supports queue eviction
/// efficiently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
    /// Block the sender until space is available.
    ///
    /// Note: `tell()` is synchronous, so Block is only effective for the test
    /// runtime's internal dispatch. Real adapters handle this natively.
    Block,
    /// Reject the message with an error.
    RejectWithError,
    /// Drop the newest message (the one being sent) silently.
    DropNewest,
}
