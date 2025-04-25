//! Metrics for the P2P stack.

use lazy_static::lazy_static;
use prometheus::{self, HistogramVec, IntGauge, register_histogram_vec, register_int_gauge};

lazy_static! {
    /// Gauge of the number of connected peers.
    pub static ref PEER_COUNT: IntGauge = register_int_gauge!(
        "kona_node_peer_count",
        "Count of currently connected p2p peers"
    ).expect("Peer count failed to register");

    /// Histogram of currently connected peer scores.
    pub static ref PEER_SCORES: HistogramVec = register_histogram_vec!(
        "kona_node_peer_scores",
        "Histogram of currently connected peer scores",
        &["type"],
        vec![-100.0, -40.0, -20.0, -10.0, -5.0, -2.0, -1.0, -0.5, -0.05, 0.0, 0.05, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 40.0]
    ).expect("Peer scores failed to register");
}
