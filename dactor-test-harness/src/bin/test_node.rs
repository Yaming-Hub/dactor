use dactor_test_harness::node::{TestNode, TestNodeConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let node_id = std::env::var("DACTOR_NODE_ID").unwrap_or_else(|_| "test-node".to_string());
    let port: u16 = std::env::var("DACTOR_CONTROL_PORT")
        .unwrap_or_else(|_| "50051".to_string())
        .parse()
        .expect("invalid port");

    let config = TestNodeConfig::from_args(&node_id, port);
    let node = TestNode::new(config);

    if let Err(e) = node.run().await {
        eprintln!("Test node error: {}", e);
        std::process::exit(1);
    }
}
