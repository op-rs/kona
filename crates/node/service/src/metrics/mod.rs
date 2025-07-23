//! Metrics for the node service

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the counter that tracks the number of times the L1 has reorganized.
    pub const L1_REORG_COUNT: &str = "kona_node_l1_reorg_count";

    /// Identifier for the counter that tracks the L1 origin of the derivation pipeline.
    pub const DERIVATION_L1_ORIGIN: &str = "kona_node_derivation_l1_origin";

    /// Identifier for the counter of critical derivation errors (strictly for alerting.)
    pub const DERIVATION_CRITICAL_ERROR: &str = "kona_node_derivation_critical_errors";

    /// Identifier for the counter that tracks sequencer state flags.
    pub const SEQUENCER_STATE: &str = "kona_node_sequencer_state";

    /// Initializes metrics for the node service.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona-node-service`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        // L1 reorg count
        metrics::describe_counter!(Self::L1_REORG_COUNT, metrics::Unit::Count, "L1 reorg count");

        // Derivation L1 origin
        metrics::describe_counter!(Self::DERIVATION_L1_ORIGIN, "Derivation pipeline L1 origin");

        // Derivation critical error
        metrics::describe_counter!(
            Self::DERIVATION_CRITICAL_ERROR,
            "Critical errors in the derivation pipeline"
        );

        // Sequencer active
        metrics::describe_counter!(Self::SEQUENCER_STATE, "Tracks sequencer state flags");
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // L1 reorg reset count
        kona_macros::set!(counter, Self::L1_REORG_COUNT, 0);

        // Derivation critical error
        kona_macros::set!(counter, Self::DERIVATION_CRITICAL_ERROR, 0);
    }
}
