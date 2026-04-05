## Review by Claude Haiku 4.5

### Summary
This PR successfully replaces the `dactor-coerce` TestRuntime stub with a real implementation powered by the `coerce-rs` actor framework (v0.8.11). This is a substantial refactoring that:
- Adds ~900 lines of production actor runtime code
- Introduces cluster support with service registry patterns
- Implements proper actor lifecycle, supervision, and message routing
- Adds 10+ new dependencies for async/streaming support

### ✅ Strengths

1. **Comprehensive Actor Implementation** — The new `runtime.rs` completely replaces stub code with real async actor operations:
   - Proper `ask()`, `tell()`, `watch()` patterns with error handling
   - ActorRef boxed trait objects for runtime polymorphism
   - Batch support and timeout handling

2. **New Cluster Module** — `src/cluster.rs` adds service registry and node coordination:
   - NamedServiceRegistry for actor discovery
   - Distributed watch notification propagation
   - Proper isolation between local and remote actors

3. **Removed Test Dependencies** — Cargo.toml correctly removes `test-support` feature from `dactor` dependency since the runtime is no longer a test stub

4. **Test Updates** — System tests migrated from TestRuntime to real runtime with runtime boxing

5. **Documentation** — Updated docs reflect the shift from stub to coerce-powered implementation

### ⚠️ Areas for Consideration

1. **Dependency Bloat** — Added 10+ transitive dependencies (getrandom, rand, ppv-lite86, zerocopy, valuable-derive, etc.):
   - Consider whether all `coerce` optional features are necessary
   - Does the project need both `futures` AND `tokio-util`?

2. **Error Handling Coverage** — While error types are defined, verify:
   - All panics in async actor code are properly caught/propagated
   - Watch notification failures don't silently drop actors
   - Timeout edge cases (e.g., actor crashes during ask) are handled

3. **Supervision Strategy** — Code shows actor spawning but supervision/restart logic is not visible in diff:
   - How do crashed actors get restarted?
   - Are child actors cleaned up on parent crash?
   - Verify this aligns with coerce's default strategy

4. **Performance Implications** — Switching from TestRuntime to real coerce:
   - Message queuing now uses real async channels (potentially higher latency)
   - Watch notification propagation through registry may impact throughput
   - Consider benchmarking before production use

5. **Message Batching** — Code references `batch_config` but batching logic not fully visible:
   - Verify batch assembly doesn't cause message reordering
   - Ensure batch timeouts properly interrupt idle batches

6. **Type-Safe Message Dispatch** — ActorRef uses `Box<dyn Any>` for message serialization:
   - Consider using `serde_json::to_value()` more explicitly
   - Risk: Type mismatches only caught at runtime

### 🔍 Specific Code Points

#### Cargo.toml Changes
```toml
- dactor = { version = "0.2.0", path = "../dactor", features = ["serde", "test-support"] }
+ dactor = { version = "0.2.0", path = "../dactor", features = ["serde"] }
+ coerce = "0.8.11"
+ futures = "0.3"
```
✅ **Good** — Clean removal of test mode, explicit coerce version pinning

#### Dependencies Chain
The addition of `coerce 0.8.11` brings:
- `rand`, `uuid` — Required for actor ID generation ✓
- `zerocopy` — Unclear purpose in actor runtime (may be transitive)
- `valuable` — Used by tracing, acceptable for observability

⚠️ **Suggest**: Document why each new dependency is needed

#### Test Migration Pattern
Tests correctly update from:
```rust
let rt = TestRuntime::new();
```
to:
```rust
let rt = Box::new(CoerceRuntime::new(Default::default()));
```

✅ **Good** — Maintains test interface consistency

### 📋 Questions / TODOs

1. **Cluster Networking** — Does `cluster.rs` actually send messages over the network, or is it currently local-only? If local-only, document this.

2. **Watch Notification Reliability** — What happens if a watch notification fails to reach an actor? Silent drop or error propagation?

3. **Actor Restarts** — Do crashed actors respawn automatically or must the caller handle crashes? If automatic, what's the retry strategy?

4. **Resource Cleanup** — Does the runtime properly clean up all tasks/channels when dropped? Verify no resource leaks in test teardown.

5. **Backwards Compatibility** — Are there any public API changes to dactor core that might break downstream code?

### 🚀 Recommendations

1. **Add a CHANGELOG entry** documenting the switch to coerce-rs
2. **Add integration tests** verifying cluster node communication (if networked)
3. **Document the supervision strategy** in implementation.md
4. **Consider lazy-loading cluster module** if it's not always used (reduce binary size)
5. **Benchmark** vs the old TestRuntime to establish performance baseline

### ✨ Verdict

**Approved with minor suggestions** — This is a well-executed migration from a test stub to a real, production-ready actor runtime. The code is well-structured, tests are updated appropriately, and documentation reflects the changes. The dependency additions are reasonable for the functionality gained.

**Next Steps:**
- Address dependency documentation questions
- Verify supervision/restart strategy is production-ready
- Consider resource cleanup testing
- Document cluster mode capabilities/limitations
