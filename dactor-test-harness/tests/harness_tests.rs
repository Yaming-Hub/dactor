use dactor_test_harness::TestCluster;
use std::time::Duration;

fn test_node_binary() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove deps
    path.push("test-node");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path.to_string_lossy().to_string()
}

#[tokio::test]
async fn test_launch_and_ping() {
    let binary = test_node_binary();
    if !std::path::Path::new(&binary).exists() {
        eprintln!("Skipping test: test-node binary not found at {}", binary);
        return;
    }

    let mut cluster = TestCluster::builder()
        .node("node-1", &binary, &[], 50061)
        .build()
        .await;

    let response = cluster.ping("node-1", "hello").await.unwrap();
    assert_eq!(response.echo, "hello");
    assert_eq!(response.node_id, "node-1");
    assert!(response.uptime_ms > 0);

    cluster.shutdown().await;
}

#[tokio::test]
async fn test_get_node_info() {
    let binary = test_node_binary();
    if !std::path::Path::new(&binary).exists() {
        return;
    }

    let mut cluster = TestCluster::builder()
        .node("info-node", &binary, &[], 50062)
        .build()
        .await;

    let info = cluster.get_node_info("info-node").await.unwrap();
    assert_eq!(info.node_id, "info-node");
    assert_eq!(info.actor_count, 0);

    cluster.shutdown().await;
}

#[tokio::test]
async fn test_inject_and_clear_faults() {
    let binary = test_node_binary();
    if !std::path::Path::new(&binary).exists() {
        return;
    }

    let mut cluster = TestCluster::builder()
        .node("fault-node", &binary, &[], 50063)
        .build()
        .await;

    cluster
        .inject_fault("fault-node", "partition", "other-node", 0, 0)
        .await
        .unwrap();

    cluster.clear_faults("fault-node").await.unwrap();

    cluster.shutdown().await;
}

#[tokio::test]
async fn test_event_subscription() {
    let binary = test_node_binary();
    if !std::path::Path::new(&binary).exists() {
        return;
    }

    let mut cluster = TestCluster::builder()
        .node("event-node", &binary, &[], 50064)
        .build()
        .await;

    let mut events = cluster.subscribe_events("event-node", &[]).await.unwrap();

    cluster
        .inject_fault("event-node", "test-fault", "target", 0, 0)
        .await
        .unwrap();

    let event = events.next_event(Duration::from_secs(5)).await;
    assert!(event.is_some());
    let event = event.unwrap();
    assert_eq!(event.event_type, "fault_injected");

    cluster.shutdown().await;
}

#[tokio::test]
async fn test_shutdown_node() {
    let binary = test_node_binary();
    if !std::path::Path::new(&binary).exists() {
        return;
    }

    let mut cluster = TestCluster::builder()
        .node("shutdown-node", &binary, &[], 50065)
        .build()
        .await;

    let response = cluster.ping("shutdown-node", "alive").await.unwrap();
    assert_eq!(response.echo, "alive");

    cluster.shutdown_node("shutdown-node").await.unwrap();
}
