//! Metrics for the P2P stack.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks gossip events.
    pub const GOSSIP_EVENT: &str = "kona_node_gossip_events";

    /// Identifier for the gauge that tracks libp2p gossipsub events.
    pub const GOSSIPSUB_EVENT: &str = "kona_node_gossipsub_events";

    /// Identifier for the gauge that tracks libp2p gossipsub connections.
    pub const GOSSIPSUB_CONNECTION: &str = "kona_node_gossipsub_connection";

    /// Identifier for the gauge that tracks unsafe blocks published.
    pub const UNSAFE_BLOCK_PUBLISHED: &str = "kona_node_unsafe_block_published";

    /// Identifier for the gauge that tracks the number of connected peers.
    pub const GOSSIP_PEER_COUNT: &str = "kona_node_swarm_peer_count";

    /// Identifier for the gauge that tracks the number of dialed peers.
    pub const DIAL_PEER: &str = "kona_node_dial_peer";

    /// Identifier for the gauge that tracks the number of errors when dialing peers.
    pub const DIAL_PEER_ERROR: &str = "kona_node_dial_peer_error";

    /// Identifier for discv5 events.
    pub const DISCOVERY_EVENT: &str = "kona_node_discovery_events";

    /// Counter for the number of FIND_NODE requests.
    pub const FIND_NODE_REQUEST: &str = "kona_node_find_node_requests";

    /// Timer for the time taken to store ENRs in the bootstore.
    pub const ENR_STORE_TIME: &str = "kona_node_enr_store_time";

    /// Identifier for the gauge that tracks the number of peers in the discovery service.
    pub const DISCOVERY_PEER_COUNT: &str = "kona_node_discovery_peer_count";

    /// Identifier for the gauge that tracks RPC calls.
    pub const RPC_CALLS: &str = "kona_node_rpc_calls";

    /// Identifier for a gauge that tracks the number of banned peers.
    pub const BANNED_PEERS: &str = "kona_node_banned_peers";

    /// Identifier for a histogram that tracks peer scores.
    pub const PEER_SCORES: &str = "kona_node_peer_scores";

    /// Identifier for the gauge that tracks the duration of peer connections in seconds.
    pub const GOSSIP_PEER_CONNECTION_DURATION_SECONDS: &str =
        "kona_node_gossip_peer_connection_duration_seconds";

    /// Initializes metrics for the P2P stack.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona_p2p`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        metrics::describe_gauge!(Self::RPC_CALLS, "Calls made to the P2P RPC module");
        metrics::describe_gauge!(
            Self::GOSSIPSUB_EVENT,
            "Events received by the libp2p gossipsub Swarm"
        );
        metrics::describe_gauge!(Self::DIAL_PEER, "Number of peers dialed by the libp2p Swarm");
        metrics::describe_gauge!(
            Self::UNSAFE_BLOCK_PUBLISHED,
            "Number of OpNetworkPayloadEnvelope gossipped out through the libp2p Swarm"
        );
        metrics::describe_gauge!(Self::DISCOVERY_EVENT, "Events received by the discv5 service");
        metrics::describe_histogram!(
            Self::ENR_STORE_TIME,
            "Observations of elapsed time to store ENRs in the on-disk bootstore"
        );
        metrics::describe_gauge!(
            Self::DISCOVERY_PEER_COUNT,
            "Number of peers connected to the discv5 service"
        );
        metrics::describe_gauge!(
            Self::FIND_NODE_REQUEST,
            "Requests made to find a node through the discv5 peer discovery service"
        );
        metrics::describe_gauge!(
            Self::GOSSIP_PEER_COUNT,
            "Number of peers connected to the libp2p gossip Swarm"
        );
        metrics::describe_gauge!(
            Self::GOSSIPSUB_CONNECTION,
            "Connections made to the libp2p Swarm"
        );
        metrics::describe_gauge!(Self::BANNED_PEERS, "Number of peers banned by kona's P2P stack");
        metrics::describe_histogram!(
            Self::PEER_SCORES,
            "Observations of peer scores in the gossipsub mesh"
        );
        metrics::describe_histogram!(
            Self::GOSSIP_PEER_CONNECTION_DURATION_SECONDS,
            "Duration of peer connections in seconds"
        );
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // RPC Calls
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_self", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_peerCount", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_peers", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_peerStats", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_discoveryTable", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_blockPeer", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_listBlockedPeers", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_blockAddr", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_unblockAddr", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_listBlockedAddrs", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_blockSubnet", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_unblockSubnet", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_listBlockedSubnets", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_protectPeer", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_unprotectPeer", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_connectPeer", 0);
        kona_macros::set!(gauge, Self::RPC_CALLS, "method", "opp2p_disconnectPeer", 0);

        // Gossip Events
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "type", "message", 0);
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "type", "subscribed", 0);
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "type", "unsubscribed", 0);
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "type", "slow_peer", 0);
        kona_macros::set!(gauge, Self::GOSSIP_EVENT, "type", "not_supported", 0);

        // Peer dials
        kona_macros::set!(gauge, Self::DIAL_PEER, 0);
        kona_macros::set!(gauge, Self::DIAL_PEER_ERROR, 0);

        // Unsafe Blocks
        kona_macros::set!(gauge, Self::UNSAFE_BLOCK_PUBLISHED, 0);

        // Discovery Event
        kona_macros::set!(gauge, Self::DISCOVERY_EVENT, "type", "discovered", 0);
        kona_macros::set!(gauge, Self::DISCOVERY_EVENT, "type", "session_established", 0);
        kona_macros::set!(gauge, Self::DISCOVERY_EVENT, "type", "unverifiable_enr", 0);

        // Peer Counts
        kona_macros::set!(gauge, Self::GOSSIP_PEER_COUNT, 0);
        kona_macros::set!(gauge, Self::DISCOVERY_PEER_COUNT, 0);
        kona_macros::set!(gauge, Self::FIND_NODE_REQUEST, 0);

        // Connection
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "connected", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "outgoing_error", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "incoming_error", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_CONNECTION, "type", "closed", 0);

        // Gossipsub Events
        kona_macros::set!(gauge, Self::GOSSIPSUB_EVENT, "type", "subscribed", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_EVENT, "type", "unsubscribed", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_EVENT, "type", "gossipsub_not_supported", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_EVENT, "type", "slow_peer", 0);
        kona_macros::set!(gauge, Self::GOSSIPSUB_EVENT, "type", "message_received", 0);

        // Banned Peers
        kona_macros::set!(gauge, Self::BANNED_PEERS, 0);
    }
}
