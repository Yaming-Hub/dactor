# dactor v0.2 — Implementation Progress

> Tracks PR status for the v0.2 implementation. See [dev-plan.md](dev-plan.md) for full plan details.

---

## Progress Summary

| Milestone | PRs | Status |
|-----------|-----|--------|
| v0.2.0-alpha.1 — Core API | PR 1–5 | 🔲 Not started |
| v0.2.0-alpha.2 — Messaging & Mailbox | PR 6–10 | 🔲 Not started |
| v0.2.0-beta.1 — Communication Complete | PR 11–14 | 🔲 Not started |
| v0.2.0-beta.2 — Error Model & Persistence | PR 15–17 | 🔲 Not started |
| v0.2.0-rc.1 — Observability & Remote | PR 18–20 | 🔲 Not started |

---

## PR Tracker

| PR | Title | Branch | Status | Tests | Notes |
|----|-------|--------|--------|-------|-------|
| 1 | Module reorganization & cleanup | | 🔲 Not started | | |
| 2 | Actor trait & ActorId | | 🔲 Not started | | |
| 3 | Message, Handler, ActorRef\<A\> | | 🔲 Not started | | |
| 4 | Tell (fire-and-forget) | | 🔲 Not started | | |
| 5 | Ask (request-reply) | | 🔲 Not started | | |
| 6 | Envelope, Headers, RuntimeHeaders | | 🔲 Not started | | |
| 7 | Interceptor pipeline (Inbound) | | 🔲 Not started | | |
| 8 | Interceptor pipeline (Outbound) | | 🔲 Not started | | |
| 9 | Lifecycle hooks & ErrorAction | | 🔲 Not started | | |
| 10 | MailboxConfig & OverflowStrategy | | 🔲 Not started | | |
| 11 | Supervision & DeathWatch | | 🔲 Not started | | |
| 12 | Stream (server-streaming) | | 🔲 Not started | | |
| 13 | Feed (client-streaming) | | 🔲 Not started | | |
| 14 | Cancellation (CancellationToken) | | 🔲 Not started | | |
| 15 | Error model (ActorError, ErrorCodec) | | 🔲 Not started | | |
| 16 | Persistence (EventSourced + DurableState) | | 🔲 Not started | | |
| 17 | Dead letters, Delay, Throttle | | 🔲 Not started | | |
| 18 | Observability (MetricsInterceptor) | | 🔲 Not started | | |
| 19 | Testing infrastructure (Conformance, MockCluster) | | 🔲 Not started | | |
| 20 | Remote actors & cluster stubs | | 🔲 Not started | | |

---

## Test Coverage

| Layer | Target | Current |
|-------|--------|---------|
| Core unit tests | ~150 | 0 |
| Adapter unit tests | ~80 | 44 (v0.1) |
| Conformance tests | ~50 | 0 |
| Integration tests | ~30 | 0 |
| **Total** | **~310** | **44** |

---

## Change Log

| Date | PR | Change |
|------|-----|--------|
| | | |
