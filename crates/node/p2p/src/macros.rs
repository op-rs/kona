//! Macros for recording metrics.

/// Sets a metric value.
#[macro_export]
macro_rules! set {
    ($metric:ident, $value:expr) => {
        #[cfg(feature = "metrics")]
        $crate::metrics::$metric.set($value);
        #[cfg(feature = "metrics")]
        tracing::info!(target: "metrics", "Set {} to {}", stringify!($metric), $value);
    };
}
