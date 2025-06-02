//! Metrics for the sources.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks runtime loader values.
    pub const RUNTIME_LOADER: &str = "kona_node_runtime_loader";

    /// Initializes metrics for the sources crate.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics to 0 so they can be queried immediately.
    #[cfg(feature = "metrics")]
    pub fn init() {
        Self::describe();
    }

    /// Describes metrics used in [`kona_sources`][crate].
    #[cfg(feature = "metrics")]
    pub fn describe() {
        metrics::describe_gauge!(Self::RUNTIME_LOADER, "Runtime configuration metadata");
    }
}
