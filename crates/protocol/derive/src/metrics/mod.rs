//! Metrics for the derivation pipeline.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the pipeline origin gauge.
    pub const PIPELINE_ORIGIN: &str = "kona_derive_pipeline_origin";

    /// Identifier to track the amount of time it takes to advance the pipeline origin.
    pub const PIPELINE_ORIGIN_ADVANCE: &str = "kona_derive_pipeline_origin_advance";

    /// Identifier for the histogram that tracks when the system config is updated.
    pub const SYSTEM_CONFIG_UPDATE: &str = "kona_derive_system_config_update";
}

impl Metrics {
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
        metrics::describe_gauge!(
            Self::PIPELINE_ORIGIN,
            "The block height of the pipeline l1 origin"
        );
        metrics::describe_histogram!(
            Self::PIPELINE_ORIGIN_ADVANCE,
            "The amount of time it takes to advance the pipeline origin"
        );
        metrics::describe_histogram!(
            Self::SYSTEM_CONFIG_UPDATE,
            "The time it takes to update the system config"
        );
    }

    /// Initializes metrics to 0 so they can be queried immediately.
    #[allow(clippy::missing_const_for_fn)]
    #[cfg(feature = "metrics")]
    pub fn zero() {}
}
