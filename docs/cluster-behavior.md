# Cluster Node Join/Leave Behavior

> How dactor manages cluster membership changes — node discovery, joining,
> leaving, and split-brain resolution.

## Overview

dactor's cluster membership is managed through three cooperating components:

```
┌─────────────────────────────────────────────────────────┐
│  ClusterDiscovery (§10.1)                               │
│  Finds peer nodes (static seeds, DNS, K8s API, etc.)    │
│  Produces: list of (NodeId, address) pairs              │
└───────────────┬─────────────────────────────────────────┘
                │ discovered peers
                ▼
┌─────────────────────────────────────────────────────────┐
│  NodeDirectory (§8.3)                                   │
│  Tracks peer status: Connecting → Connected →           │
│  Unreachable → Disconnected                             │
│  Emits: ClusterEvent::NodeJoined / NodeLeft             │
└───────────────┬─────────────────────────────────────────┘
                │ status changes
                ▼
┌─────────────────────────────────────────────────────────┐
│  ClusterEventEmitter / ClusterEvents (§10.4)            │
│  Notifies subscribers of membership changes             │
│  Actors can subscribe via runtime.cluster_events()      │
└─────────────────────────────────────────────────────────┘
```

## Node Lifecycle States

```
         ┌──────────┐
         │  Unknown  │  (not in NodeDirectory)
         └─────┬─────┘
               │ add_peer()
               ▼
         ┌──────────┐
         │Connecting │  (discovery found peer, connection in progress)
         └─────┬─────┘
               │ set_status(Connected)
               ▼                          ──► ClusterEvent::NodeJoined
         ┌──────────┐
         │Connected  │  (healthy, can exchange messages)
         └─────┬─────┘
               │ health check failure
               ▼
         ┌──────────┐
         │Unreachable│  (suspected down, retrying)
         └─────┬─────┘
               │ set_status(Disconnected) or confirmed down
               ▼                          ──► ClusterEvent::NodeLeft
         ┌──────────┐
         │Disconnected│ (removed from active cluster)
         └────────────┘
```

---

## Scenario 1: Kubernetes/VMSS Autoscale — Adding a 4th Node

### Setup
- 3 existing nodes: `node-1`, `node-2`, `node-3` (all Connected)
- Autoscaler detects high load, provisions a new pod/VM
- New node: `node-4` starts up

### Step-by-step

```
Time  Event                                  Who Does It
─────────────────────────────────────────────────────────────────

t0    Autoscaler provisions node-4 pod/VM     K8s / VMSS

t1    node-4 process starts                   OS
      → CoerceRuntime::new() / RactorRuntime::new()
      → System actors initialized
      → ClusterDiscovery starts

t2    node-4 discovers seeds                  ClusterDiscovery
      → In K8s: queries the headless Service DNS
        (e.g., dactor-cluster.default.svc.cluster.local)
      → Gets: [node-1:4697, node-2:4697, node-3:4697]
      → In VMSS: queries Azure Instance Metadata
        or reads static seed list from config

t3    node-4 connects to seeds               Transport layer
      → For each seed: transport.connect(addr)
      → Each successful connection:
        runtime.connect_peer(NodeId("node-X"), Some(addr))
        → NodeDirectory: Connecting → Connected
        → ClusterEvent::NodeJoined(node-X) emitted on node-4

t4    Existing nodes detect node-4            Discovery / Transport
      → node-1/2/3 each receive connection from node-4
      → OR: their own ClusterDiscovery re-scans DNS and finds node-4
      → Each existing node:
        runtime.connect_peer(NodeId("node-4"), Some(addr))
        → ClusterEvent::NodeJoined(node-4) emitted on each

t5    Cluster is now 4 nodes                  All nodes
      → All NodeDirectories have 3 Connected peers each
      → Actors on existing nodes receive NodeJoined events
        if subscribed via cluster_events().subscribe()
      → New actors can be spawned on node-4 via remote spawn

t6    node-4 is ready for work                Application
      → Application actors start handling messages
      → Pool routing can include node-4 workers
```

### What the application sees

```rust
// On any existing node, a subscriber registered before autoscale:
runtime.cluster_events().subscribe(Box::new(|event| {
    match event {
        ClusterEvent::NodeJoined(id) => {
            println!("New node joined: {id:?}");
            // Rebalance work, add to routing table, etc.
        }
        ClusterEvent::NodeLeft(id) => {
            println!("Node left: {id:?}");
        }
    }
}));
```

### Discovery Mechanisms by Environment

| Environment | Discovery Method | How It Works |
|------------|-----------------|--------------|
| **Kubernetes** | Headless Service DNS | DNS SRV/A records for `my-service.namespace.svc.cluster.local` return all pod IPs. Polled periodically. |
| **Azure VMSS** | Instance Metadata + Tag query | Query IMDS for VMSS instances with matching tag. Or use static seed list in config. |
| **Static** | `StaticSeeds` | Hardcoded list of `host:port` in config. New node must be added to config and existing nodes restarted (or config hot-reloaded). |
| **Consul/etcd** | Service registry | Nodes register on startup, deregister on shutdown. Discovery polls the registry. |

---

## Scenario 2: All Nodes Restart Simultaneously

### Setup
- 4-node cluster, all nodes crash or restart at the same time
  (e.g., K8s rolling restart with `maxUnavailable: 100%`, or a
  full datacenter power cycle)

### The Challenge

When all nodes restart simultaneously, no node has any cluster state.
Every node starts fresh with an empty `NodeDirectory`. There's no
"leader" to coordinate — all nodes must find each other independently.

### Step-by-step

```
Time  Event                                  Who Does It
─────────────────────────────────────────────────────────────────

t0    All 4 nodes restart                    OS / K8s
      → All NodeDirectories are empty
      → All ClusterDiscovery instances start scanning

t1    Nodes discover each other              ClusterDiscovery
      → Each node queries DNS / seed list / registry
      → Some nodes may start faster than others
      → node-1 (fastest) finds no peers initially
      → node-2 starts, finds node-1 via DNS
      → node-1 accepts connection from node-2

t2    Pairwise connections form              Transport
      → node-1 ↔ node-2 connected
      → node-3 starts, finds node-1 and node-2
      → node-3 connects to both
      → node-4 starts, connects to all 3

t3    Full mesh established                  All nodes
      → Each node eventually has all 3 peers Connected
      → NodeJoined events emitted for each new connection
      → Order may vary per node (non-deterministic)

t4    Cluster operational                    Application
      → All system actors running
      → Remote watches re-established by application
      → Actor state recovered via persistence (if configured)
```

### Key Behaviors

1. **No leader election needed** — dactor's cluster is a peer-to-peer mesh.
   Each node independently discovers and connects to peers. There's no
   coordinator node.

2. **Discovery retry** — if a node starts before others are ready, its
   connection attempts will fail. ClusterDiscovery should retry with
   backoff (e.g., every 1s, 2s, 4s... up to 30s).

3. **State is NOT preserved** — in-memory actor state, mailbox contents,
   and watch registrations are lost on restart. Applications must use
   persistence (EventSourced/DurableState) for state recovery.

4. **Actor IDs change** — each node generates new ActorId counters on
   restart. Remote references held before the restart are invalid.
   Applications should use actor naming (`runtime.lookup("my-actor")`)
   for stable references.

### Recommended Configuration

```rust
// Use a discovery mechanism that handles simultaneous startup
let discovery = RetryingDiscovery::new(
    DnsDiscovery::new("dactor.default.svc.cluster.local"),
    RetryConfig {
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        max_attempts: None, // retry forever
    },
);
```

---

## Scenario 3: Split-Brain — Two 2-Node Clusters

### The Problem

Network partition splits a 4-node cluster into two isolated groups:

```
    Partition
       ║
  ┌────╫────┐     ┌──────────┐
  │ node-1  ║     │  node-3  │
  │ node-2  ║     │  node-4  │
  │         ║     │          │
  │Cluster A║     │Cluster B │
  └────╫────┘     └──────────┘
       ║
```

Each group thinks the other nodes have left:
- Cluster A: NodeDirectory shows node-3, node-4 as Disconnected
- Cluster B: NodeDirectory shows node-1, node-2 as Disconnected

### How Split-Brain Occurs

1. **Network partition** — switch failure, firewall rule, cloud networking issue
2. **Health checks fail** — HealthChecker reports `Unreachable` for cross-partition nodes
3. **Status transitions** — Unreachable → Disconnected after timeout
4. **Events emitted** — `NodeLeft(node-3)`, `NodeLeft(node-4)` on Cluster A (and vice versa)

### Can the Clusters Merge Back?

**Yes**, when the network partition heals. The merge process:

```
Time  Event                                  Who Does It
─────────────────────────────────────────────────────────────────

t0    Network partition occurs               Infrastructure
      → node-1/2 lose connectivity to node-3/4
      → Health checks fail after timeout
      → ClusterEvent::NodeLeft emitted for disconnected nodes

t1    Both sub-clusters operate independently Application
      → Cluster A: 2 nodes, actors running
      → Cluster B: 2 nodes, actors running
      → ⚠️ DANGER: same actors may be running on both sides
        (e.g., singleton actors, sharded actors with same key)

t2    Network partition heals                Infrastructure
      → Connectivity restored between all 4 nodes

t3    Discovery detects healed partition      ClusterDiscovery
      → Periodic DNS/seed scan finds all 4 nodes again
      → OR: transport layer reconnection succeeds

t4    Nodes reconnect                        Transport
      → node-1 reconnects to node-3, node-4
      → connect_peer() called → ClusterEvent::NodeJoined
      → node-3 reconnects to node-1, node-2
      → Full mesh re-established

t5    Cluster merged — 4 nodes again         All nodes
      → All NodeDirectories show 3 Connected peers
      → Application handles NodeJoined events
```

### Split-Brain Data Conflicts

**dactor does NOT provide automatic split-brain resolution.** This is
an application-level concern. Common strategies:

| Strategy | Description | Use Case |
|----------|-------------|----------|
| **Last-Writer-Wins (LWW)** | Each write has a timestamp; latest wins on merge | Simple key-value state |
| **CRDTs** | Conflict-free data types that merge automatically | Counters, sets, maps |
| **Leader election** | One partition is "primary"; other stops processing | Singleton actors, ordered processing |
| **Fencing tokens** | Epoch-based fencing prevents stale writes | Database-backed actors |
| **Manual resolution** | Application detects conflicts and resolves | Complex business logic |

### Preventing Split-Brain

| Approach | How | Trade-off |
|----------|-----|-----------|
| **Quorum requirement** | Only process writes if >N/2 nodes reachable | Availability ↓ during partition |
| **Singleton fencing** | Epoch counter prevents stale singletons | Requires external coordination (etcd/ZK) |
| **Sharding rebalance** | On NodeJoined after heal, rebalance shard ownership | Brief unavailability during rebalance |
| **Read-only minority** | Smaller partition goes read-only | Prevents conflicting writes |

### What dactor Provides vs What the Application Must Handle

| Concern | dactor Provides | Application Must Handle |
|---------|----------------|------------------------|
| **Detection** | NodeLeft/NodeJoined events | Interpreting events as partition vs crash |
| **Reconnection** | Automatic via ClusterDiscovery | N/A (automatic) |
| **State merge** | Nothing — actors have independent state | Conflict resolution strategy |
| **Singleton safety** | Nothing — multiple instances may run | Leader election / fencing |
| **Shard ownership** | Nothing — no built-in sharding | Shard rebalance on topology change |

---

## Summary: The Cluster Membership Contract

1. **Discovery is pluggable** — dactor doesn't mandate how nodes find each
   other. StaticSeeds, DNS, cloud APIs, service registries are all valid.

2. **Connection is bidirectional** — when node A connects to node B, both
   nodes add the other to their NodeDirectory.

3. **Events are local** — `ClusterEvent::NodeJoined/NodeLeft` are emitted
   on each node independently based on that node's view of connectivity.

4. **No global consensus** — dactor does not implement Raft, Paxos, or any
   consensus protocol. Cluster membership is eventually consistent.

5. **Partition tolerance** — the system continues operating in each partition.
   Merging after heal is automatic (discovery + reconnect). Data conflicts
   are the application's responsibility.

6. **Persistence is separate** — cluster membership is runtime state only.
   Actor state persistence uses EventSourced/DurableState, which is
   orthogonal to cluster topology.
