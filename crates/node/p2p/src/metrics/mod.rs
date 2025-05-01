//! Metrics for the P2P stack.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks gossip events.
    pub const GOSSIP_EVENT: &str = "kona_node_gossip_events";

    /// Identifier for the gauge that tracks libp2p gossipsub connections.
    pub const GOSSIPSUB_CONNECTION: &str = "kona_node_gossipsub_connection";

    /// Identifier for the gauge that tracks unsafe blocks published.
    pub const UNSAFE_BLOCK_PUBLISHED: &str = "kona_node_unsafe_block_published";

    /// Identifier for the gauge that tracks the number of connected peers.
    pub const GOSSIP_PEER_COUNT: &str = "kona_node_swarm_peer_count";

    /// Identifier for the gauge that tracks the number of dialed peers.
    pub const DIAL_PEER: &str = "kona_node_dial_peer";

    /// Identifier for discv5 events.
    pub const DISCOVERY_EVENT: &str = "kona_node_discovery_events";

    /// Counter for the number of FIND_NODE requests.
    pub const FIND_NODE_REQUEST: &str = "kona_node_find_node_requests";

    /// Timer for the time taken to store ENRs in the bootstore.
    pub const ENR_STORE_TIME: &str = "kona_node_enr_store_time";

    /// Identifier for the gauge that tracks the number of peers in the discovery service.
    pub const DISCOVERY_PEER_COUNT: &str = "kona_node_discovery_peer_count";

    /// Indentifier for the gauge that tracks RPC calls.
    pub const RPC_CALLS: &str = "kona_node_rpc_calls";
}
