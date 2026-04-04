//! Per-destination outbound priority queue for remote messages.
//!
//! [`OutboundPriorityQueue`] sorts outbound [`WireEnvelope`]s by priority,
//! ensuring high-priority messages are transmitted first.
//!
//! ## Modes
//!
//! - **Lane mode** (default): 5 fixed lanes (Critical/High/Normal/Low/Background).
//!   Fast O(1) push, O(1) pop. Use `new()` or `with_config()`.
//! - **Comparer mode**: pluggable [`WireEnvelopeComparer`] for custom ordering
//!   (e.g., age-based promotion). O(1) push, O(n) pop. Use `with_comparer()`.
//!
//! ## Starvation mitigation (lane mode only)
//!
//! Set `max_consecutive` to limit pops from one lane before round-robining.

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::time::Instant;

use crate::message::Priority;
use crate::remote::WireEnvelope;

const LANE_COUNT: usize = 5;
const PRIORITY_HEADER_NAME: &str = "dactor.Priority";

// ---------------------------------------------------------------------------
// WireEnvelopeComparer trait
// ---------------------------------------------------------------------------

/// Metadata for priority comparison. The comparer sees priority + enqueue
/// time but NOT the message body.
#[derive(Debug, Clone)]
pub struct EnvelopeMetadata {
    priority: u8,
    enqueued_at: Instant,
}

impl EnvelopeMetadata {
    /// Priority value (0-255, lower = higher urgency).
    pub fn priority(&self) -> u8 {
        self.priority
    }
    /// When the envelope was enqueued.
    pub fn enqueued_at(&self) -> Instant {
        self.enqueued_at
    }
}

/// Custom ordering for outbound envelopes.
///
/// `now` is captured once per `pop()` for consistent ordering.
/// Return `Less` if `a` should be dequeued first.
pub trait WireEnvelopeComparer: Send + Sync + 'static {
    fn compare(&self, a: &EnvelopeMetadata, b: &EnvelopeMetadata, now: Instant) -> Ordering;
}

/// Default: lower numeric priority = dequeued first.
pub struct StrictPriorityWireComparer;

impl WireEnvelopeComparer for StrictPriorityWireComparer {
    fn compare(&self, a: &EnvelopeMetadata, b: &EnvelopeMetadata, _now: Instant) -> Ordering {
        a.priority.cmp(&b.priority)
    }
}

/// Age-aware: promotes envelopes past `max_age`. Among aged envelopes,
/// older wins (FIFO). Among fresh envelopes, lower priority value wins.
pub struct AgingWireComparer {
    max_age: std::time::Duration,
}

impl AgingWireComparer {
    pub fn new(max_age: std::time::Duration) -> Self {
        Self { max_age }
    }
}

impl WireEnvelopeComparer for AgingWireComparer {
    fn compare(&self, a: &EnvelopeMetadata, b: &EnvelopeMetadata, now: Instant) -> Ordering {
        let a_aged = now.duration_since(a.enqueued_at) >= self.max_age;
        let b_aged = now.duration_since(b.enqueued_at) >= self.max_age;
        match (a_aged, b_aged) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (true, true) => a.enqueued_at.cmp(&b.enqueued_at),
            (false, false) => a.priority.cmp(&b.priority),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_priority(envelope: &WireEnvelope) -> u8 {
    envelope
        .headers
        .get(PRIORITY_HEADER_NAME)
        .and_then(|bytes| bytes.first().copied())
        .unwrap_or(Priority::NORMAL.0)
}

fn lane_index_from_priority(priority: u8) -> usize {
    match priority {
        0..=63 => 0,
        64..=127 => 1,
        128..=191 => 2,
        192..=254 => 3,
        255 => 4,
    }
}

// ---------------------------------------------------------------------------
// QueuedEnvelope (comparer mode)
// ---------------------------------------------------------------------------

struct QueuedEnvelope {
    envelope: WireEnvelope,
    metadata: EnvelopeMetadata,
}

// ---------------------------------------------------------------------------
// OutboundPriorityQueue
// ---------------------------------------------------------------------------

/// Per-destination outbound priority queue.
pub struct OutboundPriorityQueue {
    // Lane mode
    lanes: [VecDeque<WireEnvelope>; LANE_COUNT],
    max_consecutive: usize,
    consecutive_from_lane: usize,
    current_lane: usize,
    // Comparer mode
    comparer: Option<Box<dyn WireEnvelopeComparer>>,
    sorted: Vec<QueuedEnvelope>,
    // Shared
    total: usize,
    capacity: usize,
}

impl OutboundPriorityQueue {
    /// Lane mode with no capacity limit.
    pub fn new() -> Self {
        Self {
            lanes: Default::default(),
            max_consecutive: 0,
            consecutive_from_lane: 0,
            current_lane: 0,
            comparer: None,
            sorted: Vec::new(),
            total: 0,
            capacity: 0,
        }
    }

    /// Lane mode with capacity + fairness.
    pub fn with_config(capacity: usize, max_consecutive: usize) -> Self {
        Self {
            capacity,
            max_consecutive,
            ..Self::new()
        }
    }

    /// Comparer mode with optional capacity.
    pub fn with_comparer(comparer: impl WireEnvelopeComparer, capacity: usize) -> Self {
        Self {
            comparer: Some(Box::new(comparer)),
            capacity,
            ..Self::new()
        }
    }

    fn is_comparer_mode(&self) -> bool {
        self.comparer.is_some()
    }

    /// Enqueue an envelope. Returns `Err(envelope)` if at capacity.
    #[allow(clippy::result_large_err)]
    pub fn push(&mut self, envelope: WireEnvelope) -> Result<(), WireEnvelope> {
        if self.capacity > 0 && self.total >= self.capacity {
            return Err(envelope);
        }
        if self.is_comparer_mode() {
            let metadata = EnvelopeMetadata {
                priority: extract_priority(&envelope),
                enqueued_at: Instant::now(),
            };
            self.sorted.push(QueuedEnvelope { envelope, metadata });
        } else {
            let lane = lane_index_from_priority(extract_priority(&envelope));
            self.lanes[lane].push_back(envelope);
        }
        self.total += 1;
        Ok(())
    }

    /// Dequeue the highest-priority envelope.
    pub fn pop(&mut self) -> Option<WireEnvelope> {
        if self.total == 0 {
            return None;
        }
        if self.comparer.is_some() {
            self.pop_comparer()
        } else if self.max_consecutive == 0 {
            self.pop_strict()
        } else {
            self.pop_fair()
        }
    }

    fn pop_comparer(&mut self) -> Option<WireEnvelope> {
        let comparer = self.comparer.as_ref()?;
        if self.sorted.is_empty() {
            return None;
        }
        let now = Instant::now();
        let mut best = 0;
        for i in 1..self.sorted.len() {
            if comparer.compare(&self.sorted[i].metadata, &self.sorted[best].metadata, now)
                == Ordering::Less
            {
                best = i;
            }
        }
        let entry = self.sorted.remove(best);
        self.total -= 1;
        Some(entry.envelope)
    }

    fn pop_strict(&mut self) -> Option<WireEnvelope> {
        for lane in &mut self.lanes {
            if let Some(envelope) = lane.pop_front() {
                self.total -= 1;
                return Some(envelope);
            }
        }
        None
    }

    fn pop_fair(&mut self) -> Option<WireEnvelope> {
        if self.consecutive_from_lane >= self.max_consecutive {
            self.consecutive_from_lane = 0;
            self.current_lane = self.next_non_empty_lane(self.current_lane + 1)?;
        } else if self.lanes[self.current_lane].is_empty() {
            self.consecutive_from_lane = 0;
            self.current_lane = self.next_non_empty_lane(0)?;
        }
        let envelope = self.lanes[self.current_lane].pop_front()?;
        self.total -= 1;
        self.consecutive_from_lane += 1;
        Some(envelope)
    }

    fn next_non_empty_lane(&self, start: usize) -> Option<usize> {
        for i in 0..LANE_COUNT {
            let idx = (start + i) % LANE_COUNT;
            if !self.lanes[idx].is_empty() {
                return Some(idx);
            }
        }
        None
    }

    /// Peek at the highest-priority envelope without removing it.
    pub fn peek(&self) -> Option<&WireEnvelope> {
        if self.is_comparer_mode() {
            if self.sorted.is_empty() {
                return None;
            }
            // For peek, we need the comparer but only have &self
            // Use a simple priority-based peek (comparer not available via &self)
            let mut best = 0;
            for i in 1..self.sorted.len() {
                if self.sorted[i].metadata.priority < self.sorted[best].metadata.priority {
                    best = i;
                }
            }
            Some(&self.sorted[best].envelope)
        } else {
            for lane in &self.lanes {
                if let Some(envelope) = lane.front() {
                    return Some(envelope);
                }
            }
            None
        }
    }

    /// Drain all envelopes in priority order.
    pub fn drain_ordered(&mut self) -> Vec<WireEnvelope> {
        let mut result = Vec::with_capacity(self.total);
        if self.is_comparer_mode() {
            // Sort by priority then enqueue time for stable ordering
            self.sorted
                .sort_by(|a, b| match a.metadata.priority.cmp(&b.metadata.priority) {
                    Ordering::Equal => a.metadata.enqueued_at.cmp(&b.metadata.enqueued_at),
                    other => other,
                });
            result.extend(self.sorted.drain(..).map(|e| e.envelope));
        } else {
            for lane in &mut self.lanes {
                result.extend(lane.drain(..));
            }
        }
        self.total = 0;
        self.consecutive_from_lane = 0;
        result
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Lane counts. Returns `[0; 5]` in comparer mode.
    pub fn lane_counts(&self) -> [usize; LANE_COUNT] {
        [
            self.lanes[0].len(),
            self.lanes[1].len(),
            self.lanes[2].len(),
            self.lanes[3].len(),
            self.lanes[4].len(),
        ]
    }

    /// Total envelopes in comparer mode's sorted vec (0 in lane mode).
    pub fn comparer_count(&self) -> usize {
        self.sorted.len()
    }

    pub fn clear(&mut self) {
        for lane in &mut self.lanes {
            lane.clear();
        }
        self.sorted.clear();
        self.total = 0;
        self.consecutive_from_lane = 0;
    }
}

impl Default for OutboundPriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interceptor::SendMode;
    use crate::message::HeaderValue;
    use crate::node::{ActorId, NodeId};
    use crate::remote::WireHeaders;

    fn envelope_with_priority(priority: Priority) -> WireEnvelope {
        let mut headers = WireHeaders::new();
        if let Some(bytes) = priority.to_bytes() {
            headers.insert(priority.header_name().to_string(), bytes);
        }
        WireEnvelope {
            target: ActorId {
                node: NodeId("n1".into()),
                local: 1,
            },
            target_name: "test".into(),
            message_type: "test::Msg".into(),
            send_mode: SendMode::Tell,
            headers,
            body: vec![priority.0],
            request_id: None,
            version: None,
        }
    }

    fn envelope_no_priority() -> WireEnvelope {
        WireEnvelope {
            target: ActorId {
                node: NodeId("n1".into()),
                local: 1,
            },
            target_name: "test".into(),
            message_type: "test::Msg".into(),
            send_mode: SendMode::Tell,
            headers: WireHeaders::new(),
            body: vec![],
            request_id: None,
            version: None,
        }
    }

    // -- Lane mode tests --

    #[test]
    fn empty_queue() {
        let mut q = OutboundPriorityQueue::new();
        assert!(q.is_empty());
        assert!(q.pop().is_none());
        assert!(q.peek().is_none());
    }

    #[test]
    fn priority_ordering() {
        let mut q = OutboundPriorityQueue::new();
        q.push(envelope_with_priority(Priority::BACKGROUND))
            .unwrap();
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        q.push(envelope_with_priority(Priority::HIGH)).unwrap();
        q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        assert_eq!(q.len(), 5);
        assert_eq!(q.pop().unwrap().body, vec![Priority::CRITICAL.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::HIGH.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::NORMAL.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::LOW.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::BACKGROUND.0]);
    }

    #[test]
    fn fifo_within_lane() {
        let mut q = OutboundPriorityQueue::new();
        let mut e1 = envelope_with_priority(Priority::NORMAL);
        e1.body = vec![1];
        let mut e2 = envelope_with_priority(Priority::NORMAL);
        e2.body = vec![2];
        q.push(e1).unwrap();
        q.push(e2).unwrap();
        assert_eq!(q.pop().unwrap().body, vec![1]);
        assert_eq!(q.pop().unwrap().body, vec![2]);
    }

    #[test]
    fn no_priority_defaults_to_normal() {
        let mut q = OutboundPriorityQueue::new();
        q.push(envelope_no_priority()).unwrap();
        assert_eq!(q.lane_counts()[2], 1);
    }

    #[test]
    fn capacity_rejects() {
        let mut q = OutboundPriorityQueue::with_config(2, 0);
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        assert!(q.push(envelope_with_priority(Priority::CRITICAL)).is_err());
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn fairness() {
        let mut q = OutboundPriorityQueue::with_config(0, 2);
        for _ in 0..5 {
            q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        }
        for _ in 0..3 {
            q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        }
        assert_eq!(q.pop().unwrap().body, vec![Priority::CRITICAL.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::CRITICAL.0]);
        // Fairness kicks in: rotates to normal
        assert_eq!(q.pop().unwrap().body, vec![Priority::NORMAL.0]);
    }

    #[test]
    fn lane_boundary_values() {
        let mut q = OutboundPriorityQueue::new();
        q.push(envelope_with_priority(Priority(63))).unwrap();
        q.push(envelope_with_priority(Priority(64))).unwrap();
        q.push(envelope_with_priority(Priority(127))).unwrap();
        q.push(envelope_with_priority(Priority(128))).unwrap();
        q.push(envelope_with_priority(Priority(191))).unwrap();
        q.push(envelope_with_priority(Priority(192))).unwrap();
        q.push(envelope_with_priority(Priority(254))).unwrap();
        q.push(envelope_with_priority(Priority(255))).unwrap();
        let counts = q.lane_counts();
        assert_eq!(counts, [1, 2, 2, 2, 1]);
    }

    #[test]
    fn drain_ordered_lane_mode() {
        let mut q = OutboundPriorityQueue::new();
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        let drained = q.drain_ordered();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].body, vec![Priority::CRITICAL.0]);
        assert_eq!(drained[1].body, vec![Priority::LOW.0]);
        assert!(q.is_empty());
    }

    #[test]
    fn clear_empties_all() {
        let mut q = OutboundPriorityQueue::new();
        q.push(envelope_with_priority(Priority::HIGH)).unwrap();
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.lane_counts(), [0, 0, 0, 0, 0]);
    }

    // -- Comparer mode tests --

    #[test]
    fn comparer_strict_priority_ordering() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();

        assert_eq!(q.len(), 3);
        assert_eq!(q.comparer_count(), 3);
        assert_eq!(q.pop().unwrap().body, vec![Priority::CRITICAL.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::NORMAL.0]);
        assert_eq!(q.pop().unwrap().body, vec![Priority::LOW.0]);
    }

    #[test]
    fn comparer_fifo_for_equal_priority() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        let mut e1 = envelope_with_priority(Priority::NORMAL);
        e1.body = vec![1];
        let mut e2 = envelope_with_priority(Priority::NORMAL);
        e2.body = vec![2];
        let mut e3 = envelope_with_priority(Priority::NORMAL);
        e3.body = vec![3];
        q.push(e1).unwrap();
        q.push(e2).unwrap();
        q.push(e3).unwrap();
        // Equal priority: FIFO order (uses remove() not swap_remove())
        assert_eq!(q.pop().unwrap().body, vec![1]);
        assert_eq!(q.pop().unwrap().body, vec![2]);
        assert_eq!(q.pop().unwrap().body, vec![3]);
    }

    #[test]
    fn comparer_peek_works() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        let peeked = q.peek().unwrap();
        assert_eq!(peeked.body, vec![Priority::CRITICAL.0]);
        assert_eq!(q.len(), 2); // peek doesn't remove
    }

    #[test]
    fn comparer_drain_ordered() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.push(envelope_with_priority(Priority::CRITICAL)).unwrap();
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        let drained = q.drain_ordered();
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].body, vec![Priority::CRITICAL.0]);
        assert_eq!(drained[1].body, vec![Priority::NORMAL.0]);
        assert_eq!(drained[2].body, vec![Priority::LOW.0]);
        assert!(q.is_empty());
    }

    #[test]
    fn comparer_clear_works() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        q.push(envelope_with_priority(Priority::HIGH)).unwrap();
        q.push(envelope_with_priority(Priority::LOW)).unwrap();
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.comparer_count(), 0);
    }

    #[test]
    fn comparer_with_capacity() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 2);
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        q.push(envelope_with_priority(Priority::NORMAL)).unwrap();
        assert!(q.push(envelope_with_priority(Priority::CRITICAL)).is_err());
    }

    #[test]
    fn comparer_lane_counts_returns_zeros() {
        let mut q = OutboundPriorityQueue::with_comparer(StrictPriorityWireComparer, 0);
        q.push(envelope_with_priority(Priority::HIGH)).unwrap();
        assert_eq!(q.lane_counts(), [0, 0, 0, 0, 0]);
        assert_eq!(q.comparer_count(), 1);
    }
}
