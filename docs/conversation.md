# dactor Design Evolution — Conversation History

> This document captures the iterative design conversations that shaped
> the dactor v0.2 design document. Each entry records the question or
> request, the key decision made, and what was added/changed.

---

## Phase 1: Project Setup (2026-03-26)

### 1.1 Extract actor framework from dstate → dactor
**Request:** Move the actor framework abstraction from the dstate project into
a standalone dactor repository with adapter crates.

**Action:** Created the dactor workspace with 3 crates:
- `dactor` — core traits (ActorRuntime, ActorRef, ClusterEvents, TimerHandle, Clock)
- `dactor-ractor` — ractor adapter (22 tests)
- `dactor-kameo` — kameo adapter (22 tests)

PR #1 created and merged. All 44 tests + 2 doc-tests passing.

### 1.2 Design document initiated
**Request:** Research Erlang, ractor, kameo, and other actor frameworks. Create
a design document for refactoring dactor into a professional framework.

**Action:** Created `docs/design-v0.2.md` with research across 6 frameworks
(Erlang/OTP, Akka, ractor, kameo, Actix, Coerce). PR #2 created.

---

## Phase 2: Design Principles (2026-03-26 — 2026-03-27)

### 2.1 Superset design principle
**Request:** The framework should be a superset of capabilities that more than
1 library supports. Adapters should report NotSupported.

**Decision:** Added the "≥2 frameworks → include" rule. Three adapter strategies:
✅ Library Native, ⚙️ Adapter Implemented, ❌ Not Supported. Added
`NotSupportedError` and `RuntimeError` enum.

### 2.2 Three-category adapter model
**Request:** Clarify exactly three possibilities per feature per adapter.

**Decision:** Standardized all matrices to use exactly 3 clean categories with
explicit "library has no X" explanations.

### 2.3 Async streaming support
**Request:** Support making a remote actor call that returns a stream of values.

**Decision:** Added `StreamRef` → later `ActorRef<A>::stream()`. `StreamSender<R>`
for the handler side, `BoxStream<R>` for the caller. Backpressure via bounded
channel, cancellation via drop.

### 2.4 Interceptor pipeline — Reject disposition
**Request:** Add Reject option besides Continue and Drop. For tell, Reject is
same as Drop; for ask, Reject becomes an error on sender side.

**Decision:** Added `Disposition::Reject(String)`. Runtime attaches interceptor
name automatically → `RuntimeError::Rejected { interceptor, reason }`.

### 2.5 InterceptContext metadata
**Request:** Interceptors should receive actor name, message type, etc.

**Decision:** Added `InterceptContext` with `actor_id`, `actor_name`,
`message_type`, `send_mode`, `remote`, `origin_node`.

### 2.6 Interceptor on_complete semantics
**Request:** Clarify what on_complete does for ask (reply value?) and stream
(per item or end of stream?).

**Decision:** `on_complete` called exactly once per message. `Outcome` enum
split into `TellSuccess`, `AskSuccess { reply: &dyn Any }`,
`HandlerError { error: ActorError }`, `StreamCompleted`, `StreamCancelled`.
Added `on_stream_item` for per-item observation.

---

## Phase 3: Error Model & Identity (2026-03-27)

### 3.1 Remote error handling
**Request:** Not every error can be serialized. How to handle remote actor
failures?

**Decision:** Created `ActorError` — structured, serializable error inspired by
gRPC Status: `ErrorCode` enum + `message` + `details: Vec<ErrorDetail>` +
`chain: Vec<String>` (source chain as strings). Three error layers: business,
runtime, infrastructure.

### 3.2 ActorId global uniqueness
**Request:** Is `ActorId(u64)` globally unique?

**Decision:** Changed to `ActorId { node: NodeId, local: u64 }`. Globally unique
without central coordinator.

### 3.3 Lifecycle hooks on Actor trait
**Request:** Will ActorLifecycle be implemented by the actor struct?

**Decision:** Merged lifecycle hooks directly into the `Actor` trait (not a
separate trait). `on_start`, `on_stop`, `on_error` with default no-ops.

---

## Phase 4: Consumer API Decision (2026-03-27)

### 4.1 API pattern analysis
**Request:** Analyze ractor vs kameo vs coerce consumer API patterns. Can we
provide multiple patterns controlled by cargo features?

**Action:** Documented all three patterns:
- Pattern A: Ractor — `ActorRef<MessageEnum>`, single handle()
- Pattern B: Kameo — `ActorRef<ActorType>`, `impl Message<M>`
- Pattern C: Coerce — `ActorRef<ActorType>`, `Handler<M>` + `Message`

### 4.2 Adopt Kameo/Coerce style
**Request:** Unify to Kameo/Coerce style since they have similar interface.

**Decision:** `ActorRef<A>` typed to actor struct. `Message` trait with `Reply`
type on the message. `Handler<M>` trait per (Actor, Message) pair. Compile-time
reply safety.

### 4.3 Proc-macro for reduced boilerplate
**Request:** Can macros make actor definition easier?

**Decision:** Designed `dactor-macros` crate with `#[dactor::actor]`,
`#[dactor::messages]`, `#[dactor::lifecycle]`. Generates Message structs +
impl blocks from method signatures.

### 4.4 Add Coerce to adapter matrix
**Request:** Add coerce to the adapter support matrix.

**Action:** Added dactor-coerce as fourth adapter with per-feature support table.

---

## Phase 5: Headers & Context (2026-03-27)

### 5.1 Remove concrete header types
**Request:** Don't add TraceContext struct. Let external crates (dcontext)
provide concrete types.

**Decision:** dactor provides only `Headers` + `HeaderValue` trait + `Priority`.
No `TraceContext`, `CorrelationId`, `Deadline`.

### 5.2 Header serialization for remote transport
**Request:** How does the runtime serialize headers to send to remote nodes?

**Decision:** Dual-layer design. Local: `TypeId`-keyed TypeMap. Remote:
`HeaderValue::to_bytes()` / `from_bytes()` with string key via `header_name()`.
`WireHeaders` for wire format. `HeaderRegistry` for deserialization.
Local-only headers (`to_bytes()=None`) are stripped. Unknown remote headers
preserved as raw bytes.

### 5.3 Interceptor names in rejection
**Request:** Interceptors should have a name for Rejected errors.

**Decision:** Added `Interceptor::name() -> &'static str`. Runtime attaches
it to `RuntimeError::Rejected { interceptor, reason }` and
`DeadLetterReason::InterceptorDrop/Reject`.

### 5.4 Interceptor access to message body and reply
**Request:** Can interceptors see tell/ask parameters and return values?

**Decision:** `on_receive` gains `message: &dyn Any`. `on_complete` gains
`reply: &dyn Any` in `AskSuccess`. `on_stream_item` gains `item: &dyn Any`.
Downcasting via `downcast_ref::<ConcreteType>()`.

---

## Phase 6: Remote Actors (2026-03-27 — 2026-03-28)

### 6.1 Remote actor call example
**Request:** Add consumer example for remote actor calls.

**Action:** Added full BankAccount example with serializable messages,
`ActorError` return, three error layers, ASCII flow diagram.

### 6.2 Serializable actor references
**Request:** Can I send ActorRef to another machine?

**Decision:** Yes. `ActorRef<A>` serializes to `ActorId` (NodeId + local).
Receiving node reconstructs remote ref with network routing. Location-
transparent.

### 6.3 Remote spawn serialization
**Request:** When spawning remotely, how is the Counter struct serialized?

**Decision:** `SpawnRequest` wire message with `type_name` + `actor_bytes`.
Remote node has `TypeRegistry` with `ActorFactory` for deserialization.
Both nodes must have same binary. Only data serialized, not code/handlers.

### 6.4 Sending to unavailable actors
**Request:** What happens when sending to an actor that's not started, stopped,
or doesn't exist?

**Decision:** Documented all 3 scenarios × 3 send modes (tell/ask/stream) ×
local/remote. Not-started: queued. Stopped/missing: `Err(ActorNotFound)`.

---

## Phase 7: Actor Construction (2026-03-28)

### 7.1 Spawn with parameters
**Request:** Can applications start actors with parameters?

**Decision:** Actor struct fields ARE the parameters. `on_start` made async
for initialization. Messages queue until `on_start` completes. Lifecycle
ordering guarantee documented.

### 7.2 Args vs State separation
**Request:** The whole actor state would need to be serializable for remote
spawn. Should we introduce construction parameters?

**Decision:** Introduced `type Args` on Actor trait. Args = serializable
construction parameters (cross wire). State = Actor struct (non-serializable
runtime resources). `Actor::create(args, deps)` builds state from args.
Runtime keeps args for restart.

### 7.3 Local dependencies (Deps)
**Request:** What if the actor needs local components like other actor refs?

**Decision:** Added `type Deps` on Actor trait. Three tiers:
1. Simple: `Args=Self, Deps=()`
2. Async init: `Args≠State, Deps=()`
3. Local deps: `Args + Deps` with `DepsFactory` for remote spawn

---

## Phase 8: Communication Patterns (2026-03-28)

### 8.1 Tell pattern missing from §6
**Request:** Section 6 should also mention tell.

**Action:** Added §6.1 Tell with API, properties, examples. Renumbered
Ask→§6.2, Stream→§6.3.

### 8.2 Outbound interceptors (sender-side)
**Request:** Introduce sender-side interceptors to stamp headers automatically,
eliminating need for tell_envelope.

**Decision:** Added `OutboundInterceptor` trait with `on_send()`. Sender-side
stamps trace context, correlation IDs, auth tokens automatically. Removed
`tell_envelope()` and `tell_with_priority()` from `ActorRef` API.

### 8.3 CancellationToken instead of _timeout methods
**Request:** Instead of ask_timeout, use CancellationToken.

**Decision:** `ask(msg, Option<CancellationToken>)` — single method.
`cancel_after(Duration)` helper. Supports timeout, explicit cancel,
hierarchical cancel.

### 8.4 Unified cancel parameter
**Request:** Instead of ask and ask_with, always add Option<CancellationToken>.

**Decision:** Single `ask(msg, cancel: Option<CancellationToken>)` and
`stream(msg, buffer, cancel: Option<CancellationToken>)`.

### 8.5 Remote cancellation
**Request:** Can CancellationToken be serialized? How does remote actor know
caller cancelled?

**Decision:** Token is NOT serializable. Adapter sends `CancelRequest` wire
message via dedicated control channel (bypasses mailbox). Remote runtime
creates local token, handler checks via `ctx.cancelled()` at `.await` points.
Cooperative cancellation — no thread interruption.

### 8.6 Cancel request priority
**Request:** Cancellation should be highest priority.

**Decision:** `CancelRequest` uses dedicated control channel, NOT the actor's
mailbox. Bypasses all queue depth and priority — delivered immediately.

### 8.7 Rename Interceptor → InboundInterceptor
**Request:** Since we have OutboundInterceptor, rename the receiver-side one.

**Action:** Renamed throughout: `InboundInterceptor`, `add_inbound_interceptor()`,
`SpawnConfig.inbound_interceptors`.

### 8.8 Outbound interceptor message access
**Request:** Outbound interceptor should also see message body via &dyn Any.

**Action:** Added `message: &dyn Any` parameter to `OutboundInterceptor::on_send()`.

### 8.9 Cancellation reason
**Request:** Can user specify reason for cancellation?

**Decision:** No — tokio's `CancellationToken` is boolean-only (cancelled or not).
Don't add reason at our layer. `ErrorCode::Cancelled` tells what happened;
the why is the caller's concern.

---

## Phase 9: Actor Execution & Pooling (2026-03-28)

### 9.1 Sequential execution guarantee
**Request:** Do all providers guarantee single-threaded execution?

**Decision:** Yes — all 6 frameworks guarantee it. Documented per-framework
mechanism. This is why `Handler::handle` takes `&mut self`.

### 9.2 Remote actor spawning
**Request:** Does dactor support spawning actors on remote nodes?

**Decision:** Added `SpawnConfig::target_node`. All 3 libraries support it.
`RemoteActor` marker trait. Full serialization flow documented.

### 9.3 Actor pooling / worker factory
**Request:** Ractor supports actor pooling (Factory/Worker). Do others?

**Decision:** 4+ of 6 frameworks support it. Added §8.4 with `PoolConfig`,
`PoolRouting` (RoundRobin, LeastLoaded, Random, KeyBased), `PoolRef<A>`,
`Keyed` trait for sticky routing.

---

## Phase 10: Cluster & Priority (2026-03-28)

### 10.1 Cluster discovery
**Request:** Most libraries don't own node discovery. Introduce a cluster
interop trait. Also check health monitoring.

**Decision:** Added §11.1 `ClusterDiscovery` trait with `ClusterEventEmitter`.
Built-in providers: Kubernetes, static, DNS SRV. Added §11.2
`NodeHealthMonitor` with heartbeat config and health states.

### 10.2 Priority enum fix
**Request:** Custom(u32)=5 is lower than Background=4 — broken.

**Decision:** Changed from enum to `Priority(u8)` struct with named constants:
CRITICAL=0, HIGH=64, NORMAL=128, LOW=192, BACKGROUND=255. Gaps allow custom
values naturally.

### 10.3 Mailbox priority scope
**Request:** Is priority per-actor or cross-actor?

**Decision:** Per-actor. Each actor has independent mailbox. No global
cross-actor priority queue.

### 10.4 Outbound network priority
**Request:** If many actors compete for network I/O, how to ensure high-priority
messages go first?

**Decision:** Two-lane outbound send queue (inspired by Akka Artery):
- Control Lane: system messages, always first
- User Lane: application messages, priority-ordered

Implemented in dactor core, not adapters. Adapters only provide
`NodeTransport::send_to_node()`.

### 10.5 Fairness policy for priority queue
**Request:** Can the framework expose a trait for applications to implement
fairness?

**Decision:** `MessageComparer` trait — a custom ordering function for the
priority queue. Receives `QueuedMessageMeta` with priority, age, message_type,
is_ask, origin_node. Default: `StrictPriorityComparer` (by priority only).
Built-in: `AgingComparer`, `WeightedComparer`. Same trait for both inbound
mailbox and outbound send queue.

### 10.6 Supervision fallback
**Request:** If the provider doesn't support supervision, what should the
framework do?

**Decision:** `ErrorAction::Restart` degrades to `Stop` with warning log.
`watch()` returns `Err(NotSupported)`. Degrade to Stop, don't panic.

### 10.7 Capability fail-fast
**Request:** If app calls unsupported function, should error or panic.

**Decision:** Returns `Err(NotSupported)` — app decides severity. Recommended
startup validation pattern. `RuntimeCapabilities` struct for pre-flight checks.

---

## Phase 11: Reviews & Polish (2026-03-28)

### 11.1 Design reviews
**Action:** Reviewed by Claude Haiku 4.5 and GPT-5.1. 22 findings fixed,
4 won't-fix. Gemini unavailable.

### 11.2 All deferred items addressed
**Request:** Address all deferred items in the doc.

**Action:** Added 7 new sections: Watch notifications (§3.15→§7.2), dead letter
handling (§5.4), message ordering (§8.2), remote serialization contract (§10.1),
conformance test suite (§13.3→§12.3), proc-macro errors (§14.3→§13.3),
observability (§12→§11).

### 11.3 Document reorganization
**Action:** Restructured from 10 sections with mega §3 (21 subsections) into
18 clean sections. Added 6+ mermaid diagrams. Fixed all section numbering.

---

## Design Principles Established

1. **Superset rule:** Include capabilities supported by ≥2 of 6 frameworks
2. **Three adapter strategies:** Library Native / Adapter Implemented / Not Supported
3. **Fail-fast:** Unsupported calls return `Err`, not panic — app decides severity
4. **Kameo/Coerce API:** `ActorRef<A>` typed to actor, `Handler<M>` per message
5. **Args/Deps/State separation:** Args cross wire, Deps resolved locally, State rebuilt
6. **Two-lane outbound:** Control always first, user priority-ordered
7. **Cooperative cancellation:** `CancellationToken` via `ctx.cancelled()` at `.await` points
8. **No opinionated context:** Headers are generic; concrete types from external crates
9. **Sequential execution:** Fundamental actor model guarantee — `&mut self` without locks
10. **MessageComparer:** Single trait for priority ordering + fairness in both mailbox and outbound queue
