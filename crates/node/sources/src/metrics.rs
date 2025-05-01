//! Metrics for the sources.

/// Container for metrics.
#[derive(Debug, Clone)]
pub struct Metrics;

impl Metrics {
    /// Identifier for the gauge that tracks runtime loader values.
    pub const RUNTIME_LOADER: &str = "kona_node_runtime_loader";
}
