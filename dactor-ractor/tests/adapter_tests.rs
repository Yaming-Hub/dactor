//! Integration tests for the dactor-ractor v0.2 adapter.
//!
//! Runs the conformance suite plus ractor-specific tests for the v0.2
//! `Actor` / `Handler<M>` / `TypedActorRef<A>` API.

use dactor::test_support::conformance::*;
use dactor_ractor::RactorRuntime;

// ---------------------------------------------------------------------------
// Conformance suite
// ---------------------------------------------------------------------------

#[tokio::test]
async fn conformance_tell_and_ask() {
    let runtime = RactorRuntime::new();
    test_tell_and_ask(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

#[tokio::test]
async fn conformance_message_ordering() {
    let runtime = RactorRuntime::new();
    test_message_ordering(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

#[tokio::test]
async fn conformance_ask_reply() {
    let runtime = RactorRuntime::new();
    test_ask_reply(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

#[tokio::test]
async fn conformance_stop() {
    let runtime = RactorRuntime::new();
    test_stop(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

#[tokio::test]
async fn conformance_unique_ids() {
    let runtime = RactorRuntime::new();
    test_unique_ids(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

#[tokio::test]
async fn conformance_actor_name() {
    let runtime = RactorRuntime::new();
    test_actor_name(|name, init| runtime.spawn::<ConformanceCounter>(name, init)).await;
}

// ---------------------------------------------------------------------------
// Ractor-specific: re-exports & default
// ---------------------------------------------------------------------------

#[test]
fn reexports_core_types() {
    let _ = dactor_ractor::dactor::NodeId("1".into());
    let _rt = dactor_ractor::RactorRuntime::new();
    let _events = dactor_ractor::RactorClusterEvents::new();
}

#[test]
fn runtime_is_default() {
    let _rt = RactorRuntime::default();
}
