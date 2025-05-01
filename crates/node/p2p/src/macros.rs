//! Macros for recording metrics.

/// Sets a metric value, optionally with a specified label.
#[macro_export]
macro_rules! set {
    ($metric:ident, $label:expr, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::gauge!($crate::metrics::$metric, "type" => $label).set($value);
    };
    ($metric:ident, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::gauge!($crate::metrics::$metric).set($value);
    };
}

/// Increments a metric value, optionally with a specified label.
#[macro_export]
macro_rules! inc {
    ($metric:ident, $label:expr) => {
        #[cfg(feature = "metrics")]
        metrics::gauge!($crate::metrics::$metric, "type" => $label).increment(1);
    };
    ($metric:ident) => {
        #[cfg(feature = "metrics")]
        metrics::gauge!($crate::metrics::$metric).increment(1);
    };
}
