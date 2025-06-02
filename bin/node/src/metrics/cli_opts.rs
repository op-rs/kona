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

    /// Initializes the CLI metrics.
    pub fn init() {
        let labels: [(&str, &str); 4] = [
            (Self::P2P_PEER_SCORING_LEVEL, "off"),
            (Self::P2P_TOPIC_SCORING_ENABLED, "false"),
            (Self::P2P_BANNING_ENABLED, "false"),
            (Self::P2P_PEER_REDIALING, "0"),
        ];
        metrics::gauge!(Self::IDENTIFIER, &labels).set(1);
    }
}
