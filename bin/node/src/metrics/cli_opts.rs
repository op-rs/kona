//! CLI Options

/// Metrics to record various CLI options.
#[derive(Debug, Clone, PartialEq)]
pub struct CliMetrics;

impl CliMetrics {
    /// The identifier for the cli metrics gauge.
    pub const IDENTIFIER: &'static str = "kona_cli_opts";

    /// The P2P Scoring level (disabled if "off").
    pub const P2P_PEER_SCORING_LEVEL: &'static str = "kona_node_peer_scoring_level";

    /// Whether P2P Topic Scoring is enabled.
    pub const P2P_TOPIC_SCORING_ENABLED: &'static str = "kona_node_topic_scoring_enabled";

    /// Whether P2P banning is enabled.
    pub const P2P_BANNING_ENABLED: &'static str = "kona_node_banning_enabled";

    /// The value for peer redialing.
    pub const P2P_PEER_REDIALING: &'static str = "kona_node_peer_redialing";

    /// Whether flood publishing is enabled.
    pub const P2P_FLOOD_PUBLISH: &'static str = "kona_node_flood_publish";

    /// The interval to send FINDNODE requests through discv5.
    pub const P2P_DISCOVERY_INTERVAL: &'static str = "kona_node_discovery_interval";

    /// The IP to advertise via P2P.
    pub const P2P_ADVERTISE_IP: &'static str = "kona_node_advertise_ip";

    /// The advertised tcp port via P2P.
    pub const P2P_ADVERTISE_TCP_PORT: &'static str = "kona_node_advertise_tcp";

    /// The advertised udp port via P2P.
    pub const P2P_ADVERTISE_UDP_PORT: &'static str = "kona_node_advertise_udp";

    /// The low-tide peer count.
    pub const P2P_PEERS_LO: &'static str = "kona_node_peers_lo";

    /// The high-tide peer count.
    pub const P2P_PEERS_HI: &'static str = "kona_node_peers_hi";

    /// The gossip mesh d option.
    pub const P2P_GOSSIP_MESH_D: &'static str = "kona_node_gossip_mesh_d";

    /// The gossip mesh d lo option.
    pub const P2P_GOSSIP_MESH_D_LO: &'static str = "kona_node_gossip_mesh_d_lo";

    /// The gossip mesh d hi option.
    pub const P2P_GOSSIP_MESH_D_HI: &'static str = "kona_node_gossip_mesh_d_hi";

    /// The gossip mesh d lazy option.
    pub const P2P_GOSSIP_MESH_D_LAZY: &'static str = "kona_node_gossip_mesh_d_lazy";

    /// The duration to ban peers.
    pub const P2P_BAN_DURATION: &'static str = "kona_node_ban_duration";

    /// Hardfork activation times.
    pub const HARDFORK_ACTIVATION_TIMES: &'static str = "kona_node_hardforks";

    /// Top-level rollup config settings.
    pub const ROLLUP_CONFIG: &'static str = "kona_node_rollup_config";
}
