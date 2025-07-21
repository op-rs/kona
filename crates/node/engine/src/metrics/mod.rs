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
    pub const INSERT_TASK_LABEL: &str = "insert";
    /// Consolidate task label.
    pub const CONSOLIDATE_TASK_LABEL: &str = "consolidate";
    /// Forkchoice task label.
    pub const FORKCHOICE_TASK_LABEL: &str = "forkchoice-update";
    /// Build task label.
    pub const BUILD_TASK_LABEL: &str = "build";
    /// Finalize task label.
    pub const FINALIZE_TASK_LABEL: &str = "finalize";

    /// Identifier for the histogram that tracks engine method call time.
    pub const ENGINE_METHOD_REQUEST_DURATION: &str = "kona_node_engine_method_request_duration";
    /// `engine_forkchoiceUpdatedV<N>` label
    pub const FORKCHOICE_UPDATE_METHOD: &str = "engine_forkchoiceUpdated";
    /// `engine_newPayloadV<N>` label.
    pub const NEW_PAYLOAD_METHOD: &str = "engine_newPayload";
    /// `engine_getPayloadV<N>` label.
    pub const GET_PAYLOAD_METHOD: &str = "engine_getPayload";

    /// Identifier for the counter that tracks the number of times the engine has been reset.
    pub const ENGINE_RESET_COUNT: &str = "kona_node_engine_reset_count";

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

        // Engine method request duration histogram
        metrics::describe_histogram!(
            Self::ENGINE_METHOD_REQUEST_DURATION,
            metrics::Unit::Seconds,
            "Engine method request duration"
        );

        // Engine reset counter
        metrics::describe_counter!(
            Self::ENGINE_RESET_COUNT,
            metrics::Unit::Count,
            "Engine reset count"
        );
    }

    /// Initializes metrics to `0` so they can be queried immediately by consumers of prometheus
    /// metrics.
    #[cfg(feature = "metrics")]
    pub fn zero() {
        // Engine task counts
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::INSERT_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::CONSOLIDATE_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::FORKCHOICE_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::BUILD_TASK_LABEL, 0);
        kona_macros::set!(counter, Self::ENGINE_TASK_COUNT, Self::FINALIZE_TASK_LABEL, 0);

        // Engine reset count
        kona_macros::set!(counter, Self::ENGINE_RESET_COUNT, 0);
    }
}
