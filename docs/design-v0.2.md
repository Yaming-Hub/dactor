# dactor v0.2 — Design Document

> **Goal:** Refactor dactor from a minimal trait extraction into a professional,
> production-grade abstract actor framework, informed by Erlang/OTP, Akka,
> ractor, kameo, Actix, and Coerce.

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

### Key Takeaways

1. **Every framework** has tell (fire-and-forget) — this is the fundamental operation.
2. **Most frameworks** also support ask (request-reply) — we should abstract it.
3. **All production frameworks** have lifecycle hooks — we need `on_start`/`on_stop`.
4. **Supervision** is universal in Erlang, Akka, ractor, kameo — we should model it.
5. **Message envelopes** with headers exist in Akka and distributed messaging — no Rust framework does this natively yet (opportunity for dactor).
6. **Interceptors/middleware** exist in Akka (`Behaviors.intercept`) and web frameworks — we should add a message pipeline.
7. **Test support behind feature flags** is standard practice in Rust crates.

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
    fn tell_envelope(&self, envelope: Envelope<M>) -> Result<(), ActorSendError>;

    /// Check if the actor is still alive.
    fn is_alive(&self) -> bool;
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
pub trait AskRef<M, R>: ActorRef<M>
where
    M: Send + 'static,
    R: Send + 'static,
{
    /// Send a message and await a reply.
    fn ask(&self, msg: M) -> Result<tokio::sync::oneshot::Receiver<R>, ActorSendError>;
}
```

### 3.5 Actor Lifecycle

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

### 3.6 Supervision

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
    fn watch<M: Send + 'static>(
        &self,
        watcher: &Self::Ref<M>,
        target: ActorId,
    ) -> Result<(), ActorSendError>;

    /// Stop watching an actor.
    fn unwatch<M: Send + 'static>(
        &self,
        watcher: &Self::Ref<M>,
        target: ActorId,
    ) -> Result<(), ActorSendError>;
}
```

### 3.7 Mailbox Configuration

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

### 3.8 Spawn Configuration

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

### 3.9 Feature-Gated Test Support

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

### 3.10 Revised `ActorRuntime` Trait (Complete)

```rust
pub trait ActorRuntime: Send + Sync + 'static {
    type Ref<M: Send + 'static>: ActorRef<M>;
    type Events: ClusterEvents;
    type Timer: TimerHandle;

    // ── Spawning ────────────────────────────────────────
    fn spawn<M, H>(&self, name: &str, handler: H) -> Self::Ref<M>
    where M: Send + 'static, H: FnMut(M) + Send + 'static;

    fn spawn_with_config<M, H>(
        &self, name: &str, config: SpawnConfig, handler: H,
    ) -> Self::Ref<M>
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
    ) -> Result<(), GroupError>;

    fn leave_group<M: Send + 'static>(
        &self, group: &str, actor: &Self::Ref<M>,
    ) -> Result<(), GroupError>;

    fn broadcast_group<M: Clone + Send + 'static>(
        &self, group: &str, msg: M,
    ) -> Result<(), GroupError>;

    fn get_group_members<M: Send + 'static>(
        &self, group: &str,
    ) -> Result<Vec<Self::Ref<M>>, GroupError>;

    // ── Cluster ─────────────────────────────────────────
    fn cluster_events(&self) -> &Self::Events;

    // ── Global Interceptors ─────────────────────────────
    fn add_interceptor(&self, interceptor: Box<dyn Interceptor>);
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
├── clock.rs             ← Clock, SystemClock (NO TestClock)
├── cluster.rs           ← ClusterEvents, ClusterEvent, NodeId, SubscriptionId
├── timer.rs             ← TimerHandle
├── mailbox.rs           ← MailboxConfig, OverflowStrategy
├── errors.rs            ← ActorSendError, GroupError, ClusterError
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
| — | `Envelope<M>`, `Headers` | New types, `tell()` accepts both `M` and `Envelope<M>`. |
| — | `Interceptor` pipeline | New opt-in feature, no breakage. |
| — | `SpawnConfig` / `MailboxConfig` | New, with defaults matching v0.1 behavior. |
| — | `ActorLifecycle` | New opt-in trait. |

---

## 6. Adapter Impact

### dactor-ractor

| Feature | Implementation |
|---------|---------------|
| `ActorRef::id()` | Map to `ractor::ActorRef::get_id()` → `ActorId` |
| `tell_envelope()` | Unwrap envelope, pass `body` to ractor `cast()`, apply interceptors |
| `is_alive()` | Check if ractor actor cell is alive |
| `MailboxConfig` | ractor uses unbounded; bounded would need a wrapper channel |
| `Interceptor` | Run interceptor chain before calling ractor `cast()` |
| `ActorLifecycle` | Map to ractor's `pre_start` / `post_stop` callbacks |

### dactor-kameo

| Feature | Implementation |
|---------|---------------|
| `ActorRef::id()` | Map from `kameo::actor::ActorId` → `ActorId` |
| `tell_envelope()` | Unwrap envelope, pass `body` to kameo `tell().try_send()` |
| `is_alive()` | Check kameo actor ref validity |
| `MailboxConfig` | kameo natively supports bounded (`spawn_bounded`) and unbounded |
| `Interceptor` | Run interceptor chain before calling kameo `tell()` |
| `ActorLifecycle` | Map to kameo's `on_start` / `on_stop` |

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

### Phase 4 — Ask Pattern (v0.3.1)
1. Add `AskRef<M, R>` trait
2. Implement for adapters that support it
3. Add timeout support

---

## 9. Open Questions

1. **Should `Envelope<M>` be the only way to send messages?** Or should `tell(M)` auto-wrap in an envelope with empty headers? → **Proposed: both.** `tell(msg)` wraps automatically; `tell_envelope(env)` gives full control.

2. **Should interceptors be per-runtime or per-actor?** → **Proposed: both.** Global interceptors via `runtime.add_interceptor()`, per-actor via `SpawnConfig`.

3. **Should `ActorLifecycle` be a separate trait or methods on the handler?** → **Proposed: separate trait.** Simple actors use closures (no lifecycle); stateful actors implement `ActorLifecycle`.

4. **Should dactor provide a `Registry` (named actor lookup)?** → **Deferred to v0.4.** Adapters already have their own registry mechanisms.

5. **How to handle `serde` for `NodeId`?** → **Make it a feature.** `NodeId` gets `Serialize/Deserialize` only with `features = ["serde"]`.
