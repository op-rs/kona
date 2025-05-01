//! Macros for recording metrics.

/// Sets a metric value, optionally with a specified label.
#[macro_export]
macro_rules! set {
    ($instrument:ident, $metric:ident, $key:expr, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric, $key => $value).set($amount);
    };
    ($instrument:ident, $metric:ident, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric, "type" => $value).set($amount);
    };
    ($instrument:ident, $metric:ident, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric).set($value);
    };
}

/// Increments a metric value, optionally with a specified label.
#[macro_export]
macro_rules! inc {
    ($instrument:ident, $metric:ident, $key:expr, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric, $key => $value).increment(1);
    };
    ($instrument:ident, $metric:ident) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric).increment(1);
    };
}

/// Records a value, optionally with a specified label.
#[macro_export]
macro_rules! record {
    ($instrument:ident, $metric:ident, $key:expr, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric, $key => $value).record($amount);
    };
    ($instrument:ident, $metric:ident, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($crate::Metrics::$metric).record($amount);
    };
}
