//! Metrics for the engine

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks block labels.
    pub const BLOCK_LABELS: &str = "kona_node_block_labels";
    /// Unsafe block label.
    pub const UNSAFE_BLOCK_LABEL: &str = "unsafe";
    /// Cross-unsafe block label.
    pub const CROSS_UNSAFE_BLOCK_LABEL: &str = "cross-unsafe";
    /// Local-safe block label.
    pub const LOCAL_SAFE_BLOCK_LABEL: &str = "local-safe";
    /// Safe block label.
    pub const SAFE_BLOCK_LABEL: &str = "safe";
    /// Finalized block label.
    pub const FINALIZED_BLOCK_LABEL: &str = "finalized";

    /// Identifier for the counter that records engine task counts.
    pub const ENGINE_TASK_COUNT: &str = "kona_node_engine_task_count";
    /// Insert task label.
    pub const INSERT_TASK_LABEL: &str = "insert-unsafe";
    /// Consolidate task label.
    pub const CONSOLIDATE_TASK_LABEL: &str = "consolidate";
    /// Forkchoice task label.
    pub const FORKCHOICE_TASK_LABEL: &str = "forkchoice-update";
    /// Build task label.
    pub const BUILD_TASK_LABEL: &str = "build";

    /// Initializes metrics for the engine.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in [`kona_engine`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        // Block labels
        metrics::describe_gauge!(Self::BLOCK_LABELS, "Blockchain head labels");

        // Engine task counts
        metrics::describe_counter!(Self::ENGINE_TASK_COUNT, "Engine task counts");
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // Blockchain head labels
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", Self::UNSAFE_BLOCK_LABEL, 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", Self::CROSS_UNSAFE_BLOCK_LABEL, 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", Self::LOCAL_SAFE_BLOCK_LABEL, 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", Self::SAFE_BLOCK_LABEL, 0);
        kona_macros::set!(gauge, Self::BLOCK_LABELS, "label", Self::FINALIZED_BLOCK_LABEL, 0);

        // Engine task counts
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::INSERT_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::CONSOLIDATE_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::FORKCHOICE_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::BUILD_TASK_LABEL, 0);
    }
}
