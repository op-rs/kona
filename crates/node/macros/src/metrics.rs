//! Macros for recording metrics.

/// Sets a metric value, optionally with a specified label.
#[macro_export]
macro_rules! set {
    (counter, $metric:path, $key:expr, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::counter!($metric, $key => $value).absolute($amount);
    };
    ($instrument:ident, $metric:path, $key:expr, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric, $key => $value).set($amount);
    };
    (counter, $metric:path, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::counter!($metric, "type" => $value).absolute($amount);
    };
    ($instrument:ident, $metric:path, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric, "type" => $value).set($amount);
    };
    ($instrument:ident, $metric:path, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric).set($value);
    };
}

/// Increments a metric value, optionally with a specified label.
#[macro_export]
macro_rules! inc {
    ($instrument:ident, $metric:path, $key:expr, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric, $key => $value).increment(1);
    };
    ($instrument:ident, $metric:path, $value:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric, "type" => $value).increment(1);
    };
    ($instrument:ident, $metric:path) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric).increment(1);
    };
}

/// Records a value, optionally with a specified label.
#[macro_export]
macro_rules! record {
    ($instrument:ident, $metric:path, $key:expr, $value:expr, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric, $key => $value).record($amount);
    };
    ($instrument:ident, $metric:path, $amount:expr) => {
        #[cfg(feature = "metrics")]
        metrics::$instrument!($metric).record($amount);
    };
}
