# dactor v0.2 — Design Document

> **Goal:** Refactor dactor from a minimal trait extraction into a professional,
> production-grade abstract actor framework, informed by Erlang/OTP, Akka,
> ractor, kameo, Actix, and Coerce.

---

## 0. Design Principle: Superset with Graceful Degradation

### Inclusion Rule

**dactor abstracts the superset of capabilities supported by 2 or more actor
frameworks.** If a behavior is common to at least two of the surveyed
frameworks, dactor models it as a first-class trait or type. Individual adapters
that don't natively support a capability have two options:

1. **Adapter-layer implementation** — the adapter implements the capability
   using custom logic (e.g., ractor doesn't have bounded mailboxes, but the
   adapter can wrap a bounded channel).
2. **`NotSupported` error** — the adapter returns `Err(NotSupported)` at
   runtime, signaling the caller that this capability is unavailable with the
   chosen backend.

This ensures the **core API is rich and forward-looking** while each adapter
remains honest about what it can deliver.

### Capability Inclusion Matrix

The table below counts how many of the 6 surveyed frameworks support each
capability. **≥ 2** means it qualifies for inclusion in dactor.

| Capability | Erlang | Akka | Ractor | Kameo | Actix | Coerce | Count | Include? |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Tell (fire-and-forget) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| Ask (request-reply) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| Typed messages | — | ✓ | ✓ | ✓ | ✓ | ✓ | **5** | ✅ |
| Actor identity (ID) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| Lifecycle hooks | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| Supervision | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| DeathWatch / monitoring | ✓ | ✓ | ✓ | ✓ | — | — | **4** | ✅ |
| Timers (send_after/interval) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **6** | ✅ |
| Processing groups | ✓ | ✓ | ✓ | — | — | ✓ | **4** | ✅ |
| Actor registry (named lookup) | ✓ | ✓ | ✓ | — | ✓ | ✓ | **5** | ✅ (v0.4) |
| Mailbox configuration | — | ✓ | — | ✓ | ✓ | — | **3** | ✅ |
| Interceptors / middleware | — | ✓ | — | — | ✓ | ✓ | **3** | ✅ |
| Message envelope / metadata | ✓ | ✓ | — | — | — | — | **2** | ✅ |
| Cluster events | ✓ | ✓ | ✓ | ✓ | — | ✓ | **5** | ✅ |
| Distribution (remote actors) | ✓ | ✓ | ✓ | ✓ | — | ✓ | **5** | ✅ (future) |
| Clock abstraction | ✓ | ✓ | — | — | — | — | **2** | ✅ |
| Streaming responses | ✓ | ✓ | — | — | — | — | **2** | ✅ |
| Hot code upgrade | ✓ | — | — | — | — | — | **1** | ❌ |

### `NotSupported` Error

All trait methods that might not be supported by every adapter return a
`Result` type. A new error variant is introduced:

```rust
/// Error indicating that the adapter does not support this operation.
#[derive(Debug, Clone)]
pub struct NotSupportedError {
    /// Name of the operation that is not supported.
    pub operation: &'static str,
    /// Name of the adapter/runtime that doesn't support it.
    pub adapter: &'static str,
    /// Optional detail message.
    pub detail: Option<String>,
}

impl fmt::Display for NotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not supported by {}", self.operation, self.adapter)?;
        if let Some(ref detail) = self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for NotSupportedError {}
```

A unified error enum encompasses all runtime errors:

```rust
/// Unified error type for all ActorRuntime operations.
#[derive(Debug)]
pub enum RuntimeError {
    /// The actor's mailbox is closed or the send failed.
    Send(ActorSendError),
    /// Processing group operation failed.
    Group(GroupError),
    /// Cluster event operation failed.
    Cluster(ClusterError),
    /// The requested operation is not supported by this adapter.
    NotSupported(NotSupportedError),
}
```

### Adapter Support Matrix (Planned)

| Capability | dactor-ractor | dactor-kameo | Strategy |
|---|:---:|:---:|---|
| `tell()` | ✅ native | ✅ native | — |
| `tell_envelope()` | ✅ adapter | ✅ adapter | Unwrap envelope, run interceptors, forward body |
| `ask()` | ✅ native (`call`) | ✅ native (`ask`) | Map to framework's request-reply |
| `ActorRef::id()` | ✅ native | ✅ native | Map from framework's ID type |
| `ActorRef::is_alive()` | ✅ native | ✅ native | Check actor cell liveness |
| Lifecycle hooks | ✅ native | ✅ native | Map to `pre_start`/`post_stop` and `on_start`/`on_stop` |
| Supervision | ✅ native | ✅ native (`on_link_died`) | Map to framework's supervision model |
| `watch()` / `unwatch()` | ✅ native | ✅ native (linking) | Map to framework's monitoring API |
| `MailboxConfig::Bounded` | ⚙️ adapter | ✅ native (`spawn_bounded`) | ractor: wrap with bounded channel |
| `MailboxConfig::Unbounded` | ✅ native | ✅ native | — |
| `OverflowStrategy::Block` | ⚙️ adapter | ✅ native | ractor: bounded channel blocks |
| `OverflowStrategy::DropOldest` | ❌ `NotSupported` | ❌ `NotSupported` | Neither framework supports natively |
| Interceptors (global) | ✅ adapter | ✅ adapter | Interceptor chain runs in adapter before dispatch |
| Interceptors (per-actor) | ✅ adapter | ✅ adapter | Stored in SpawnConfig, run per message |
| Processing groups | ✅ adapter | ✅ adapter | Already implemented in v0.1 |
| `add_interceptor()` | ✅ adapter | ✅ adapter | Store in `Arc<Mutex<Vec>>` on runtime |
| `stream()` | ⚙️ adapter | ⚙️ adapter | Channel-based: actor sends items via `mpsc::Sender`, caller gets `StreamRef` |
| Cluster events | ✅ adapter | ✅ adapter | Already implemented in v0.1 |

**Legend:** ✅ native = framework provides direct API; ⚙️ adapter = implemented
with custom adapter logic; ❌ `NotSupported` = returns error at runtime.

---

## 1. Research Summary: Common Behaviors Across Actor Frameworks

| Concept | Erlang/OTP | Akka (JVM) | Ractor (Rust) | Kameo (Rust) | Actix (Rust) | Coerce (Rust) |
|---|---|---|---|---|---|---|
| **Message passing** | `Pid ! Msg` (async) | tell `!` / ask `?` | `cast()` / `call()` | `tell()` / `ask()` | `do_send()` / `send()` | `notify()` / `send()` |
| **Typed messages** | Dynamic (any term) | Typed behaviors | `ActorRef<M>` | `Message<M>` trait | `Handler<M>` trait | `Handler<M>` trait |
| **Lifecycle hooks** | `init`, `terminate`, `handle_info` | `preStart`, `postStop`, `preRestart` | `pre_start`, `post_start`, `post_stop` | `on_start`, `on_stop`, `on_panic` | `started()`, `stopped()` | Lifecycle events |
| **Supervision** | Supervisor trees with strategies | Resume/Restart/Stop/Escalate | Parent-child supervision | `on_link_died` linking | Built-in supervision | Supervision + clustering |
| **DeathWatch** | `monitor/2` | `context.watch()` → `Terminated` | Supervisor notifications | Actor linking | — | — |
| **Interceptors** | — | `Behaviors.intercept` | — | — | Middleware (web) | Metrics/tracing |
| **Message envelope** | Built-in (pid, ref, msg) | Envelope with metadata | Plain typed msg | Plain typed msg | Plain typed msg | Plain typed msg |
| **Timers** | `send_after`, `send_interval` | Scheduler | tokio tasks | tokio tasks | `run_interval` | tokio tasks |
| **Processing groups** | `pg` module | Cluster-aware routing | Named groups | — | — | Pub/sub, sharding |
| **Actor registry** | `register/2` (named) | Receptionist | Named registry | — | Registry | Actor system |
| **Mailbox config** | Per-process (unbounded) | Bounded/custom | Unbounded | Bounded (default) | Bounded | Unbounded |
| **Distribution** | Native (Erlang nodes) | Akka Cluster/Remoting | `ractor_cluster` | libp2p / Kademlia | — | Cluster, remote actors |
| **Clock/time** | `erlang:monotonic_time` | Scheduler | — | — | — | — |
| **Streaming responses** | Multi-part `gen_server` reply | Akka Streams `Source` | — | — | — | — |

### Key Takeaways

1. **Every framework** has tell (fire-and-forget) — this is the fundamental operation.
2. **Most frameworks** also support ask (request-reply) — we should abstract it.
3. **All production frameworks** have lifecycle hooks — we need `on_start`/`on_stop`.
4. **Supervision** is universal in Erlang, Akka, ractor, kameo — we should model it.
5. **Message envelopes** with headers exist in Erlang and Akka (2 frameworks) — qualifies for inclusion under the superset rule.
6. **Interceptors/middleware** exist in Akka, Actix, and Coerce (3 frameworks) — qualifies for inclusion.
7. **Test support behind feature flags** is standard practice in Rust crates.
8. **Superset rule applied:** every capability above is supported by ≥ 2 frameworks (see §0). Adapters return `NotSupported` for features they can't provide.
9. **Streaming responses** exist in Erlang (multi-part `gen_server` replies) and Akka (Akka Streams `Source`) — qualifies under the superset rule. Combined with the ubiquity of gRPC server-streaming and Rust's async `Stream` trait, this is a high-value addition.

---

## 2. Current dactor Architecture (v0.1)

```
dactor/
├── traits/
│   ├── runtime.rs    → ActorRuntime, ActorRef<M>, ClusterEvents, TimerHandle
│   └── clock.rs      → Clock, SystemClock, TestClock
├── types/
│   └── node.rs       → NodeId
└── test_support/
    ├── test_runtime.rs → TestRuntime, TestActorRef, TestClusterEvents
    └── test_clock.rs   → re-export of TestClock
```

### Problems

1. **`TestClock` lives in `traits/clock.rs`** — test utilities mixed with production code
2. **No feature gate** on test_support — always compiled
3. **Messages are plain `M`** — no metadata, no tracing context, no correlation
4. **No interceptor pipeline** — can't inject logging, metrics, or context propagation
5. **Only fire-and-forget** — no ask/reply pattern
6. **No lifecycle hooks** — actors are just closures with no start/stop semantics
7. **No supervision** — no way to monitor or restart actors
8. **No actor identity** — actors have no ID, can't be compared or addressed by name
9. **No mailbox configuration** — adapter decides (ractor=unbounded, kameo=bounded)
10. **`NodeId` uses `serde`** — unnecessary dependency for the core crate unless needed

---

## 3. Proposed Architecture (v0.2)

```
dactor/
├── src/
│   ├── lib.rs
│   ├── actor.rs           → ActorRef, ActorId, ActorRuntime trait
│   ├── message.rs         → Envelope<M>, MessageHeaders, Header trait
│   ├── interceptor.rs     → Interceptor trait, InterceptorChain
│   ├── lifecycle.rs       → ActorLifecycle hooks
│   ├── supervision.rs     → Supervisor trait, SupervisionStrategy
│   ├── clock.rs           → Clock, SystemClock (production only)
│   ├── cluster.rs         → ClusterEvents, ClusterEvent, NodeId
│   ├── timer.rs           → TimerHandle
│   ├── mailbox.rs         → MailboxConfig
│   └── errors.rs          → All error types
├── src/test_support/      → behind #[cfg(feature = "test-support")]
│   ├── mod.rs
│   ├── test_runtime.rs
│   └── test_clock.rs
```

### 3.1 Message Envelope

**Rationale:** Every distributed system eventually needs message metadata — trace
IDs, correlation IDs, deadlines, security context. Baking this into the
framework from day one avoids a breaking change later.

```rust
/// Type-erased header value. Adapters can store trace context,
/// correlation IDs, deadlines, auth tokens, etc.
pub trait HeaderValue: Send + Sync + 'static {
    fn as_any(&self) -> &dyn std::any::Any;
}

/// A collection of typed headers attached to a message.
///
/// Headers use `TypeId`-keyed storage so that each header type can be
/// inserted and retrieved without string lookups or downcasting guesswork.
#[derive(Default)]
pub struct Headers { /* TypeMap internally */ }

impl Headers {
    pub fn insert<H: HeaderValue>(&mut self, value: H);
    pub fn get<H: HeaderValue>(&self) -> Option<&H>;
    pub fn remove<H: HeaderValue>(&mut self) -> Option<H>;
    pub fn is_empty(&self) -> bool;
}

/// An envelope wrapping a message body with typed headers.
pub struct Envelope<M> {
    pub headers: Headers,
    pub body: M,
}

impl<M> From<M> for Envelope<M> {
    fn from(body: M) -> Self {
        Envelope { headers: Headers::default(), body }
    }
}
```

**Built-in header types** (provided by dactor, opt-in):

```rust
/// W3C-compatible trace context for distributed tracing.
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub trace_flags: u8,
}

/// Correlation ID for request tracking across actors.
pub struct CorrelationId(pub String);

/// Deadline after which the message should be discarded.
pub struct Deadline(pub std::time::Instant);
```

### 3.2 Interceptor Pipeline

**Rationale:** Akka has `Behaviors.intercept`, HTTP frameworks have middleware.
An interceptor pipeline lets users add cross-cutting concerns (logging,
metrics, tracing, auth) without modifying actor code.

```rust
/// Outcome of an interceptor's processing.
pub enum Disposition {
    /// Continue to the next interceptor / deliver the message.
    Continue,
    /// Drop the message silently (e.g., rate-limiting, auth failure).
    Drop,
}

/// An interceptor that can observe or modify messages in transit.
///
/// Interceptors form an ordered pipeline. Each interceptor sees the
/// envelope before the actor's handler and can modify headers, log,
/// record metrics, or drop the message entirely.
pub trait Interceptor: Send + Sync + 'static {
    /// Called before the message is delivered to the actor.
    fn on_receive(&self, headers: &mut Headers) -> Disposition {
        Disposition::Continue
    }

    /// Called after the actor's handler returns (for post-processing).
    fn on_complete(&self, headers: &Headers) {}
}
```

**Example: Logging interceptor**

```rust
struct LoggingInterceptor;

impl Interceptor for LoggingInterceptor {
    fn on_receive(&self, headers: &mut Headers) -> Disposition {
        if let Some(cid) = headers.get::<CorrelationId>() {
            tracing::info!(correlation_id = %cid.0, "message received");
        }
        Disposition::Continue
    }
}
```

**Registration:**

```rust
// On the runtime:
runtime.add_interceptor(LoggingInterceptor);

// Or per-actor at spawn time:
runtime.spawn_with_config("my-actor", config, handler);
```

### 3.3 ActorRef & ActorId

**Rationale:** Every framework gives actors identity. Without an ID, you can't
implement supervision, death watch, logging, or debugging.

```rust
/// Unique identifier for an actor within a runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId(pub u64);

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Actor({})", self.0)
    }
}

pub trait ActorRef<M: Send + 'static>: Clone + Send + Sync + 'static {
    /// The actor's unique identity.
    fn id(&self) -> ActorId;

    /// Fire-and-forget: deliver a raw message.
    fn tell(&self, msg: M) -> Result<(), ActorSendError>;

    /// Fire-and-forget with an envelope (headers + body).
    /// Adapters that don't support envelopes may ignore headers and
    /// forward only the body, or return `NotSupported` if the envelope
    /// cannot be delivered at all.
    fn tell_envelope(&self, envelope: Envelope<M>) -> Result<(), RuntimeError>;

    /// Check if the actor is still alive.
    /// Returns `Err(NotSupported)` if the adapter cannot determine liveness.
    fn is_alive(&self) -> Result<bool, NotSupportedError>;
}
```

> **Note:** `send()` is renamed to `tell()` to align with Erlang/Akka/kameo
> terminology. A deprecated `send()` alias can ease migration.

### 3.4 Ask Pattern (Request-Reply)

**Rationale:** ractor, kameo, Akka, and Actix all support ask. Not having it
forces users to implement reply channels manually.

The ask pattern is modeled as a **separate trait** so adapters that don't
support it can omit the implementation:

```rust
/// Extension trait for request-reply messaging.
///
/// Adapters that support ask natively (ractor `call`, kameo `ask`)
/// implement this trait. Adapters that don't support it should provide
/// a blanket implementation returning `NotSupported`.
pub trait AskRef<M, R>: ActorRef<M>
where
    M: Send + 'static,
    R: Send + 'static,
{
    /// Send a message and await a reply.
    /// Returns `Err(RuntimeError::NotSupported)` if the adapter doesn't
    /// support request-reply messaging.
    fn ask(&self, msg: M) -> Result<tokio::sync::oneshot::Receiver<R>, RuntimeError>;
}
```

### 3.5 Streaming (Request-Stream)

**Rationale:** Erlang supports multi-part `gen_server` replies where a server
sends chunked results back to the caller over time. Akka has first-class
support via Akka Streams `Source`, tightly integrated with actors. gRPC server
streaming is the dominant RPC pattern for streaming data. In Rust, the async
`Stream` trait (`futures_core::Stream`) is the standard abstraction, and
`tokio::sync::mpsc` channels convert naturally into streams via
`tokio_stream::wrappers::ReceiverStream`.

dactor should provide a `stream()` method on actor references that sends a
request to an actor and returns an async stream of response items. This enables
use cases like:

- Paginated data retrieval
- Real-time event feeds / subscriptions
- Long-running computation with progressive results
- Fan-out aggregation with incremental delivery

**Core types:**

```rust
use std::pin::Pin;
use futures_core::Stream;

/// A pinned, boxed, Send-safe async stream of items.
/// This is the return type from `StreamRef::stream()` — the caller
/// consumes it with `while let Some(item) = stream.next().await`.
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

/// A sender handle given to the actor's stream handler.
/// The actor pushes items into this sender; the caller receives them
/// as an async stream on the other end.
///
/// Backed by a bounded `mpsc` channel for backpressure.
pub struct StreamSender<T: Send + 'static> {
    inner: tokio::sync::mpsc::Sender<T>,
}

impl<T: Send + 'static> StreamSender<T> {
    /// Send an item to the stream consumer.
    /// Returns `Err` if the consumer has dropped the stream.
    pub async fn send(&self, item: T) -> Result<(), StreamSendError> {
        self.inner.send(item).await
            .map_err(|_| StreamSendError::ConsumerDropped)
    }

    /// Try to send without blocking. Returns `Err` if the channel is
    /// full or the consumer has dropped.
    pub fn try_send(&self, item: T) -> Result<(), StreamSendError> {
        self.inner.try_send(item)
            .map_err(|e| match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) =>
                    StreamSendError::Full,
                tokio::sync::mpsc::error::TrySendError::Closed(_) =>
                    StreamSendError::ConsumerDropped,
            })
    }

    /// Check if the consumer is still listening.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[derive(Debug)]
pub enum StreamSendError {
    /// The consumer dropped the stream (no longer reading).
    ConsumerDropped,
    /// The channel buffer is full (backpressure).
    Full,
}
```

**Extension trait on `ActorRef`:**

```rust
/// Extension trait for request-stream messaging.
///
/// The caller sends a request message and receives a stream of response
/// items. The actor's handler receives the message together with a
/// `StreamSender<R>` and pushes items into it.
///
/// Adapters implement this using a bounded `mpsc` channel: the adapter
/// creates the channel, wraps the `Receiver` into a `BoxStream`, and
/// passes the `Sender` (as `StreamSender<R>`) to the actor alongside
/// the request message.
pub trait StreamRef<M, R>: ActorRef<M>
where
    M: Send + 'static,
    R: Send + 'static,
{
    /// Send a request and receive a stream of responses.
    ///
    /// `buffer` controls the channel capacity (backpressure). A typical
    /// default is 16 or 32.
    ///
    /// Returns `Err(RuntimeError::NotSupported)` if the adapter doesn't
    /// support streaming.
    fn stream(
        &self,
        msg: M,
        buffer: usize,
    ) -> Result<BoxStream<R>, RuntimeError>;
}
```

**How it works (adapter implementation pattern):**

```
Caller                       Adapter Layer                     Actor
  │                               │                              │
  │  stream(request, buf=16)      │                              │
  │──────────────────────────────►│                              │
  │                               │  create mpsc(16)             │
  │                               │  tx = StreamSender(sender)   │
  │                               │  rx = ReceiverStream(recv)   │
  │                               │                              │
  │                               │  deliver (request, tx)       │
  │                               │─────────────────────────────►│
  │◄─ return BoxStream(rx)        │                              │
  │                               │                              │
  │  .next().await ◄──────────────│◄── tx.send(item_1) ─────────│
  │  .next().await ◄──────────────│◄── tx.send(item_2) ─────────│
  │  .next().await ◄──────────────│◄── tx.send(item_3) ─────────│
  │  None (stream ends) ◄────────│◄── drop(tx) ─────────────────│
```

**Backpressure:** The bounded channel naturally provides backpressure. If the
caller is slow to consume, the actor's `tx.send().await` will suspend until
the caller reads an item, preventing unbounded memory growth.

**Cancellation:** When the caller drops the `BoxStream`, the `Receiver` is
dropped, closing the channel. The actor's next `tx.send()` returns
`StreamSendError::ConsumerDropped`, signaling it to stop producing.

**Example usage (caller side):**

```rust
use dactor::{ActorRuntime, StreamRef};
use tokio_stream::StreamExt;

async fn get_logs(runtime: &impl ActorRuntime, actor: &impl StreamRef<GetLogs, LogEntry>) {
    let mut stream = actor.stream(GetLogs { since: yesterday() }, 32).unwrap();

    while let Some(entry) = stream.next().await {
        println!("{}: {}", entry.timestamp, entry.message);
    }
}
```

**Example usage (actor handler side):**

```rust
// The actor receives a tuple of (request, StreamSender)
// when dispatched via stream(). The adapter wraps the handler.
async fn handle_get_logs(request: GetLogs, tx: StreamSender<LogEntry>) {
    for entry in database.query_logs(request.since).await {
        if tx.send(entry).await.is_err() {
            break; // consumer dropped the stream
        }
    }
    // dropping tx closes the stream on the caller side
}
```

**Relationship to Ask:** `ask()` is request → single reply. `stream()` is
request → multiple replies. Both are modeled as separate extension traits
(`AskRef` and `StreamRef`) so that adapters can implement either, both, or
neither independently.

**Dependencies:** The core crate adds `futures-core` (for the `Stream` trait)
and `tokio-stream` (for `ReceiverStream`) as dependencies, both lightweight
and standard in the async Rust ecosystem.

### 3.6 Actor Lifecycle

**Rationale:** Erlang has `init/terminate`, Akka has `preStart/postStop`,
ractor has `pre_start/post_stop`, kameo has `on_start/on_stop`.

```rust
/// Lifecycle hooks for an actor. All methods have default no-op
/// implementations so that simple actors can ignore them.
pub trait ActorLifecycle: Send + 'static {
    /// Called after the actor is spawned, before it processes any messages.
    fn on_start(&mut self) {}

    /// Called when the actor is stopping (graceful shutdown).
    fn on_stop(&mut self) {}

    /// Called when the actor's handler panics or returns an error.
    /// Return an `ErrorAction` to control what happens next.
    fn on_error(&mut self, _error: Box<dyn std::error::Error + Send>) -> ErrorAction {
        ErrorAction::Stop
    }
}

pub enum ErrorAction {
    /// Resume processing the next message (Erlang: continue).
    Resume,
    /// Restart the actor (Erlang: restart, Akka: Restart).
    Restart,
    /// Stop the actor (Erlang: shutdown).
    Stop,
    /// Escalate to the supervisor (Akka: Escalate).
    Escalate,
}
```

### 3.7 Supervision

**Rationale:** Erlang supervisors, Akka supervision strategies, ractor
parent-child supervision, kameo `on_link_died`.

```rust
/// Notification sent to a supervisor when a child actor terminates.
pub struct ChildTerminated {
    pub child_id: ActorId,
    pub child_name: String,
    /// `None` for graceful shutdown, `Some(reason)` for failure.
    pub reason: Option<String>,
}

/// Strategy applied by a supervisor when a child fails.
pub trait SupervisionStrategy: Send + Sync + 'static {
    fn on_child_failed(&self, event: &ChildTerminated) -> SupervisionAction;
}

pub enum SupervisionAction {
    /// Restart the failed child actor.
    Restart,
    /// Stop the failed child and don't restart.
    Stop,
    /// Escalate the failure to the parent supervisor.
    Escalate,
}

/// Built-in strategies (matching Erlang/Akka conventions):
pub struct OneForOne;       // restart only the failed child
pub struct OneForAll;       // restart all children when one fails
pub struct RestForOne;      // restart the failed child and all after it
```

**DeathWatch** — any actor can watch another:

```rust
pub trait ActorRuntime: Send + Sync + 'static {
    // ... existing methods ...

    /// Watch an actor. When it terminates, the watcher receives a
    /// `ChildTerminated` notification via its message handler.
    /// Returns `Err(NotSupported)` if the adapter doesn't support death watch.
    fn watch<M: Send + 'static>(
        &self,
        watcher: &Self::Ref<M>,
        target: ActorId,
    ) -> Result<(), RuntimeError>;

    /// Stop watching an actor.
    /// Returns `Err(NotSupported)` if the adapter doesn't support death watch.
    fn unwatch<M: Send + 'static>(
        &self,
        watcher: &Self::Ref<M>,
        target: ActorId,
    ) -> Result<(), RuntimeError>;
}
```

### 3.8 Mailbox Configuration

**Rationale:** kameo defaults to bounded, ractor to unbounded. The abstraction
should let users choose.

```rust
/// Mailbox sizing strategy for an actor.
#[derive(Debug, Clone)]
pub enum MailboxConfig {
    /// Unbounded mailbox — never blocks senders. Risk of memory exhaustion.
    Unbounded,
    /// Bounded mailbox with backpressure.
    Bounded {
        capacity: usize,
        /// What to do when the mailbox is full.
        overflow: OverflowStrategy,
    },
}

pub enum OverflowStrategy {
    /// Block the sender until space is available.
    Block,
    /// Drop the newest message (the one being sent).
    DropNewest,
    /// Drop the oldest message in the mailbox.
    DropOldest,
    /// Return an error to the sender.
    RejectWithError,
}

impl Default for MailboxConfig {
    fn default() -> Self {
        MailboxConfig::Unbounded
    }
}
```

### 3.9 Spawn Configuration

Collect all per-actor settings into a config struct:

```rust
pub struct SpawnConfig {
    pub mailbox: MailboxConfig,
    pub interceptors: Vec<Box<dyn Interceptor>>,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            mailbox: MailboxConfig::default(),
            interceptors: Vec::new(),
        }
    }
}
```

Updated `ActorRuntime::spawn`:

```rust
pub trait ActorRuntime: Send + Sync + 'static {
    // Simple spawn (backward-compatible)
    fn spawn<M, H>(&self, name: &str, handler: H) -> Self::Ref<M>
    where
        M: Send + 'static,
        H: FnMut(M) + Send + 'static;

    // Spawn with configuration
    fn spawn_with_config<M, H>(
        &self,
        name: &str,
        config: SpawnConfig,
        handler: H,
    ) -> Self::Ref<M>
    where
        M: Send + 'static,
        H: FnMut(M) + Send + 'static;

    // ... timers, groups, cluster events ...
}
```

### 3.10 Feature-Gated Test Support

**Rationale:** `TestClock`, `TestRuntime`, `TestClusterEvents` are test
utilities. They should not be compiled into production binaries.

```toml
# dactor/Cargo.toml
[features]
default = []
test-support = ["tokio/test-util"]
```

```rust
// dactor/src/lib.rs
#[cfg(feature = "test-support")]
pub mod test_support;
```

Downstream crates use:

```toml
[dev-dependencies]
dactor = { version = "0.2", features = ["test-support"] }
```

### 3.11 Revised `ActorRuntime` Trait (Complete)

```rust
pub trait ActorRuntime: Send + Sync + 'static {
    type Ref<M: Send + 'static>: ActorRef<M>;
    type Events: ClusterEvents;
    type Timer: TimerHandle;

    // ── Spawning ────────────────────────────────────────
    fn spawn<M, H>(&self, name: &str, handler: H) -> Self::Ref<M>
    where M: Send + 'static, H: FnMut(M) + Send + 'static;

    /// Spawn with per-actor configuration (mailbox, interceptors).
    /// Returns `Err(NotSupported)` for config options the adapter can't honor.
    fn spawn_with_config<M, H>(
        &self, name: &str, config: SpawnConfig, handler: H,
    ) -> Result<Self::Ref<M>, RuntimeError>
    where M: Send + 'static, H: FnMut(M) + Send + 'static;

    // ── Timers ──────────────────────────────────────────
    fn send_interval<M: Clone + Send + 'static>(
        &self, target: &Self::Ref<M>, interval: Duration, msg: M,
    ) -> Self::Timer;

    fn send_after<M: Send + 'static>(
        &self, target: &Self::Ref<M>, delay: Duration, msg: M,
    ) -> Self::Timer;

    // ── Processing Groups ───────────────────────────────
    fn join_group<M: Send + 'static>(
        &self, group: &str, actor: &Self::Ref<M>,
    ) -> Result<(), RuntimeError>;

    fn leave_group<M: Send + 'static>(
        &self, group: &str, actor: &Self::Ref<M>,
    ) -> Result<(), RuntimeError>;

    fn broadcast_group<M: Clone + Send + 'static>(
        &self, group: &str, msg: M,
    ) -> Result<(), RuntimeError>;

    fn get_group_members<M: Send + 'static>(
        &self, group: &str,
    ) -> Result<Vec<Self::Ref<M>>, RuntimeError>;

    // ── Supervision / DeathWatch ────────────────────────
    /// Watch an actor for termination.
    /// Returns `Err(NotSupported)` if the adapter doesn't support it.
    fn watch<M: Send + 'static>(
        &self, watcher: &Self::Ref<M>, target: ActorId,
    ) -> Result<(), RuntimeError>;

    fn unwatch<M: Send + 'static>(
        &self, watcher: &Self::Ref<M>, target: ActorId,
    ) -> Result<(), RuntimeError>;

    // ── Cluster ─────────────────────────────────────────
    fn cluster_events(&self) -> &Self::Events;

    // ── Global Interceptors ─────────────────────────────
    /// Register a global interceptor applied to all actors.
    /// Returns `Err(NotSupported)` if the adapter doesn't support interceptors.
    fn add_interceptor(&self, interceptor: Box<dyn Interceptor>) -> Result<(), RuntimeError>;
}
```

### 3.12 Default Implementations via `RuntimeError::NotSupported`

Capabilities that are universally available (tell, spawn, timers) never return
`NotSupported`. Capabilities that may not be available in every adapter
(watch, ask, certain mailbox configs) return `Result<_, RuntimeError>`.

Adapters have three strategies for each method:

| Strategy | When to use | Example |
|---|---|---|
| **Native mapping** | The underlying framework directly supports the operation | ractor `call()` → dactor `ask()` |
| **Adapter-layer shim** | The framework lacks the API but the adapter can emulate it | ractor bounded mailbox via wrapper channel |
| **`NotSupported`** | The feature genuinely can't be provided | `OverflowStrategy::DropOldest` on ractor |

```rust
// Example: adapter that doesn't support watch
fn watch<M: Send + 'static>(
    &self, _watcher: &Self::Ref<M>, _target: ActorId,
) -> Result<(), RuntimeError> {
    Err(RuntimeError::NotSupported(NotSupportedError {
        operation: "watch",
        adapter: "dactor-example",
        detail: Some("underlying framework has no death watch API".into()),
    }))
}
```

---

## 4. Module Reorganization

### Before (v0.1)

```
dactor/src/
├── lib.rs
├── traits/
│   ├── mod.rs
│   ├── runtime.rs       ← ActorRef + ActorRuntime + errors + ClusterEvents + NodeId
│   └── clock.rs         ← Clock + SystemClock + TestClock (mixed!)
├── types/
│   ├── mod.rs
│   └── node.rs          ← NodeId
└── test_support/
    ├── mod.rs
    ├── test_runtime.rs
    └── test_clock.rs
```

### After (v0.2)

```
dactor/src/
├── lib.rs               ← public API, re-exports
├── actor.rs             ← ActorRef, ActorId, ActorRuntime, SpawnConfig
├── message.rs           ← Envelope, Headers, HeaderValue, built-in headers
├── interceptor.rs       ← Interceptor trait, Disposition
├── lifecycle.rs         ← ActorLifecycle, ErrorAction
├── supervision.rs       ← SupervisionStrategy, SupervisionAction, ChildTerminated
├── stream.rs            ← StreamRef, StreamSender, BoxStream, StreamSendError
├── clock.rs             ← Clock, SystemClock (NO TestClock)
├── cluster.rs           ← ClusterEvents, ClusterEvent, NodeId, SubscriptionId
├── timer.rs             ← TimerHandle
├── mailbox.rs           ← MailboxConfig, OverflowStrategy
├── errors.rs            ← ActorSendError, GroupError, ClusterError, RuntimeError
└── test_support/        ← #[cfg(feature = "test-support")]
    ├── mod.rs
    ├── test_runtime.rs  ← TestRuntime, TestActorRef, TestClusterEvents
    └── test_clock.rs    ← TestClock
```

---

## 5. Breaking Changes & Migration

| v0.1 | v0.2 | Migration |
|------|------|-----------|
| `ActorRef::send()` | `ActorRef::tell()` | Rename. Provide deprecated `send()` shim for one release. |
| `TestClock` in `traits::clock` | `test_support::TestClock` behind feature | Add `features = ["test-support"]` to dev-deps. |
| `test_support` always compiled | Feature-gated | Same as above. |
| No `ActorId` | `ActorRef::id()` required | Adapters must implement. |
| `NodeId` in `types::node` | `cluster::NodeId` | Module moved, re-exported from root. |
| `GroupError` return type | `RuntimeError` return type | Wrap existing errors in `RuntimeError::Group(...)`. |
| — | `NotSupportedError` / `RuntimeError` | New error types. Unsupported ops return `Err(NotSupported)`. |
| — | `Envelope<M>`, `Headers` | New types, `tell()` accepts both `M` and `Envelope<M>`. |
| — | `Interceptor` pipeline | New opt-in feature, no breakage. |
| — | `SpawnConfig` / `MailboxConfig` | New, with defaults matching v0.1 behavior. |
| — | `ActorLifecycle` | New opt-in trait. |
| — | `StreamRef<M, R>` / `BoxStream<R>` | New streaming trait. Adapters implement via channel shim. |

---

## 6. Adapter Impact

### Strategy Key

- ✅ **Native** — direct 1:1 mapping to framework API
- ⚙️ **Adapter shim** — implemented in adapter layer with custom logic
- ❌ **`NotSupported`** — returns `RuntimeError::NotSupported` at runtime

### dactor-ractor

| Feature | Strategy | Implementation |
|---------|:---:|---|
| `tell()` | ✅ | `ractor::ActorRef::cast()` |
| `tell_envelope()` | ⚙️ | Run interceptor chain on headers, forward `body` to `cast()` |
| `ask()` | ✅ | `ractor::ActorRef::call()` |
| `ActorRef::id()` | ✅ | Map `ractor::ActorRef::get_id()` → `ActorId` |
| `ActorRef::is_alive()` | ✅ | Check ractor actor cell liveness |
| Lifecycle hooks | ✅ | Map to ractor `pre_start` / `post_stop` |
| Supervision | ✅ | Map to ractor's parent-child supervision |
| `watch()` / `unwatch()` | ✅ | Map to ractor's supervisor notification system |
| `MailboxConfig::Unbounded` | ✅ | Default ractor behavior |
| `MailboxConfig::Bounded` | ⚙️ | Wrap with bounded `tokio::sync::mpsc` channel |
| `OverflowStrategy::Block` | ⚙️ | Bounded channel naturally blocks sender |
| `OverflowStrategy::RejectWithError` | ⚙️ | `try_send()` on bounded channel |
| `OverflowStrategy::DropNewest` | ⚙️ | `try_send()`, discard on error |
| `OverflowStrategy::DropOldest` | ❌ | Returns `NotSupported` — no efficient way to evict from front |
| Interceptors (global) | ⚙️ | `Arc<Mutex<Vec<Box<dyn Interceptor>>>>` on runtime, run before `cast()` |
| Interceptors (per-actor) | ⚙️ | Store in actor wrapper, run per message |
| Processing groups | ⚙️ | Already implemented in v0.1 (type-erased registry) |
| `stream()` | ⚙️ | Create `mpsc` channel, pass `StreamSender` to actor with request, return `ReceiverStream` |
| Cluster events | ⚙️ | Already implemented in v0.1 (`RactorClusterEvents`) |

### dactor-kameo

| Feature | Strategy | Implementation |
|---------|:---:|---|
| `tell()` | ✅ | `kameo::ActorRef::tell().try_send()` |
| `tell_envelope()` | ⚙️ | Run interceptor chain on headers, forward `body` to `tell()` |
| `ask()` | ✅ | `kameo::ActorRef::ask()` |
| `ActorRef::id()` | ✅ | Map `kameo::actor::ActorId` → `ActorId` |
| `ActorRef::is_alive()` | ✅ | Check kameo actor ref validity |
| Lifecycle hooks | ✅ | Map to kameo `on_start` / `on_stop` |
| Supervision | ✅ | Map to kameo `on_link_died` linking model |
| `watch()` / `unwatch()` | ✅ | Map to kameo actor linking |
| `MailboxConfig::Unbounded` | ✅ | `kameo::actor::Spawn::spawn()` |
| `MailboxConfig::Bounded` | ✅ | `kameo::actor::Spawn::spawn_bounded(capacity)` |
| `OverflowStrategy::Block` | ✅ | kameo bounded mailbox default behavior |
| `OverflowStrategy::RejectWithError` | ✅ | `try_send()` returns error when full |
| `OverflowStrategy::DropNewest` | ⚙️ | `try_send()`, silently discard on error |
| `OverflowStrategy::DropOldest` | ❌ | Returns `NotSupported` — kameo doesn't expose queue eviction |
| Interceptors (global) | ⚙️ | `Arc<Mutex<Vec<Box<dyn Interceptor>>>>` on runtime |
| Interceptors (per-actor) | ⚙️ | Store in actor wrapper, run per message |
| Processing groups | ⚙️ | Already implemented in v0.1 (type-erased registry) |
| `stream()` | ⚙️ | Create `mpsc` channel, pass `StreamSender` to actor with request, return `ReceiverStream` |
| Cluster events | ⚙️ | Already implemented in v0.1 (`KameoClusterEvents`) |

---

## 7. Dependency Cleanup

### v0.1 dactor/Cargo.toml deps:
```toml
serde = { version = "1", features = ["derive"] }   # for NodeId
tokio = { version = "1", features = ["time", "sync", "rt", "macros"] }
tracing = "0.1"
```

### v0.2 proposed:
```toml
[dependencies]
tokio = { version = "1", features = ["time", "sync", "rt"] }  # drop macros
tracing = "0.1"
futures-core = "0.3"      # Stream trait (used by StreamRef)
tokio-stream = "0.1"      # ReceiverStream wrapper

[dependencies.serde]
version = "1"
features = ["derive"]
optional = true  # only if user needs NodeId serialization

[features]
default = []
serde = ["dep:serde"]
test-support = ["tokio/test-util"]
```

---

## 8. Implementation Priority

### Phase 1 — Foundation (v0.2.0)
1. Module reorganization (flat structure, one concept per file)
2. Feature-gate `test_support` behind `test-support`
3. Move `TestClock` out of `traits/clock.rs`
4. Add `ActorId` to `ActorRef` trait
5. Rename `send()` → `tell()` (with deprecated alias)
6. Add `Envelope<M>` and `Headers`
7. Add `Interceptor` trait and pipeline
8. Clean up `serde` dependency (make optional)

### Phase 2 — Lifecycle & Config (v0.2.1)
1. Add `ActorLifecycle` trait (`on_start`, `on_stop`, `on_error`)
2. Add `MailboxConfig` and `OverflowStrategy`
3. Add `SpawnConfig` for per-actor configuration
4. Add `spawn_with_config()` to `ActorRuntime`
5. Update adapter crates

### Phase 3 — Supervision (v0.3.0)
1. Add `SupervisionStrategy` trait
2. Add `ChildTerminated` event
3. Add `watch()` / `unwatch()` to `ActorRuntime`
4. Built-in strategies: `OneForOne`, `OneForAll`, `RestForOne`
5. Add `ErrorAction::Escalate` flow

### Phase 4 — Ask Pattern & Streaming (v0.3.1)
1. Add `AskRef<M, R>` trait
2. Add `StreamRef<M, R>` trait, `StreamSender<R>`, `BoxStream<R>`
3. Implement for adapters (channel-based shim)
4. Add timeout support for ask
5. Add `futures-core` and `tokio-stream` dependencies

---

## 9. Open Questions

1. **Should `Envelope<M>` be the only way to send messages?** Or should `tell(M)` auto-wrap in an envelope with empty headers? → **Proposed: both.** `tell(msg)` wraps automatically; `tell_envelope(env)` gives full control.

2. **Should interceptors be per-runtime or per-actor?** → **Proposed: both.** Global interceptors via `runtime.add_interceptor()`, per-actor via `SpawnConfig`.

3. **Should `ActorLifecycle` be a separate trait or methods on the handler?** → **Proposed: separate trait.** Simple actors use closures (no lifecycle); stateful actors implement `ActorLifecycle`.

4. **Should dactor provide a `Registry` (named actor lookup)?** → **Deferred to v0.4.** 5 of 6 frameworks support it (qualifies under superset rule), but adapters already have their own registry mechanisms. Design to be informed by adapter experience.

5. **How to handle `serde` for `NodeId`?** → **Make it a feature.** `NodeId` gets `Serialize/Deserialize` only with `features = ["serde"]`.

6. **Should `NotSupported` be a compile-time or runtime error?** → **Runtime.** Rust's trait system with GATs can't express "this adapter supports method X" at the type level without fragmenting the trait hierarchy. A single `ActorRuntime` trait with `Result<_, RuntimeError>` is simpler and more ergonomic. Adapters document their support matrix.

7. **What's the threshold for adapter-level shims vs NotSupported?** → **Effort and correctness.** If the adapter can implement the feature correctly with reasonable overhead (e.g., bounded channel wrapper for ractor), use a shim. If the emulation would be incorrect, surprising, or prohibitively expensive (e.g., DropOldest requires draining a queue), return `NotSupported`.

8. **Should `stream()` support bidirectional streaming?** → **Deferred.** Start with request-stream (one request, many responses). Bidirectional streaming (many-to-many) can be added later as a separate `BidiStreamRef` trait if there is demand. The channel-based approach naturally extends to this.
