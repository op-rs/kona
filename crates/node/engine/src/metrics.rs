#[cfg(feature = "metrics")]
pub(crate) const ENGINE_UNSAFE_HEAD_HEIGHT_NAME: &str = "engine_unsafe_head_height";

/// Describes all metrics an engine instance may produce.
///
/// This function should be called once during application startup if metric
/// descriptions are desired for observers like Prometheus. It's guarded by
/// the "metrics" feature flag.
pub fn describe_engine_metrics() {
    #[cfg(feature = "metrics")]
    {
        #[cfg(feature = "metrics")]
        metrics::describe_gauge!(
            ENGINE_UNSAFE_HEAD_HEIGHT_NAME,
            metrics::Unit::Count,
            "The block number of the most recent unsafe L2 head known to the engine."
        );
    }
}
