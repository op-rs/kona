//! Metrics for the engine

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks block labels.
    pub const BLOCK_LABELS: &str = "kona_node_block_labels";

    /// Initializes metrics for the engine.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona_engine`][crate].
    pub fn describe() {
        metrics::describe_gauge!(Self::BLOCK_LABELS, "Blockchain head labels");
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    pub fn zero() {
        // Blockchain head labels
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", "unsafe", 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", "cross-unsafe", 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", "local-safe", 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", "safe", 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", "finalized", 0);
    }
}
