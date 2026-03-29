# dactor v0.2 Design Document — Review 3

> Reviews conducted on 2026-03-29 using Claude Haiku 4.5, GPT-5.1, and GPT-5.4-mini.

---

## Consensus Findings (all reviewers agree)

| Finding | Severity | Section |
|---|---|---|
| `SpawnConfig` field name mismatch: `inbound_interceptors` vs `interceptors` in example | 🟡 | §4.7 / §5.2 |
| `ClusterEvents` / `ClusterEvent` referenced but never defined | 🔴 | §4.5 |
| `TimerHandle` referenced but never defined | 🟡 | §4.5 |
| `Outcome` enum differs between §5.2 (TellSuccess/AskSuccess) and §9.1 (Success) | 🟡 | §5.2 / §9.1 |
| Appendix A still references `ActorRef<M>` as Layer 1 core — contradicts §4 decision | 🟡 | Appendix A |
| `ValidationErrorCodec` example doesn't match new `ErrorCodec<E>` / `ErasedErrorCodec` API | 🟡 | §9.1 |

## Per-Reviewer Findings

### Haiku

| Finding | Severity |
|---|---|
| `ActorRef<M>` in research comparison table (§3) — ractor row shows message-typed | 🟢 Info (describes ractor, not dactor) |
| `NodeId` type referenced 96 times but never defined | 🔴 |
| `rgb()` color in §9.1 mermaid diagram may not render | 🟡 |
| `Outcome` enum needs single authoritative definition | 🟡 |

### GPT-5.1

| Finding | Severity |
|---|---|
| Layer diagram in Appendix A says `ActorRef<M>` is core — stale | 🟡 |
| `ValidationErrorCodec` impl/registration example stale | 🟡 |
| All mermaid diagrams verified clean (no stateDiagram, no invalid syntax) | ✅ |

### GPT-5.4-mini

| Finding | Severity |
|---|---|
| `ActorRef<M>` in Appendix A.5 "Layer 1 core" — contradicts §4 | 🟡 |
| `ErrorCodec` example still uses old `impl ErrorCodec for` not `impl ErrorCodec<E> for` | 🟡 |
| `header_name_static()` called in HeaderRegistry but trait only defines `header_name(&self)` | 🟡 |
| `PriorityFunction` referenced in §8.1 MailboxConfig but never defined | 🟡 |
| `WatchRequest`/`UnwatchRequest` mentioned in §10.2 but never defined | 🟢 |
| `HM` node reference in §10.1 mermaid diagram — node removed but ref remains | 🔴 |
| `Result‹Receipt, ActorError›` uses unusual guillemets in §11.4 mermaid | 🟢 |
| Complete Example (§4.8) uses `fn on_start(&mut self)` but trait requires `async fn on_start(&mut self, ctx)` | 🟡 |

---

## Action Items

1. ✅ Fix `SpawnConfig` field name in §5.2 example: `interceptors` → `inbound_interceptors`
2. ✅ Add `NodeId` definition in §4.4
3. ✅ Add `ClusterEvents` / `ClusterEvent` trait/enum definition
4. ✅ Add `TimerHandle` trait definition
5. ✅ Unify `Outcome` enum — use §5.2 detailed version as authoritative, remove §9.1 duplicate
6. ✅ Update Appendix A to note it's pre-decision analysis (ActorRef<M> was the old approach)
7. ✅ Fix `ValidationErrorCodec` example to use `ErrorCodec<ValidationErrors>` + `erased()` registration
8. ✅ Remove stale `HM` from §10.1 mermaid diagram
9. ✅ Fix `rgb()` in §9.1 mermaid if problematic
10. ✅ Fix §4.8 Complete Example to use `async fn on_start(&mut self, ctx: &mut ActorContext)`
11. ✅ Remove `PriorityFunction` reference from §8.1 if stale
12. ✅ Add `header_name_static()` or fix to `header_name(&self)` consistently
