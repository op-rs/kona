//! Metrics for the Managed Mode RPC client.

/// Container for metrics.
#[derive(Debug, Clone)]
pub(super) struct Metrics;

impl Metrics {
    // --- Metric Names ---
    /// Identifier for the counter of successful RPC requests. Labels: `method`.
    pub(crate) const MANAGED_MODE_RPC_REQUESTS_SUCCESS_TOTAL: &'static str =
        "managed_mode_rpc_requests_success_total";
    /// Identifier for the counter of failed RPC requests. Labels: `method`.
    pub(crate) const MANAGED_MODE_RPC_REQUESTS_ERROR_TOTAL: &'static str =
        "managed_mode_rpc_requests_error_total";
    /// Identifier for the histogram of RPC request durations. Labels: `method`.
    pub(crate) const MANAGED_MODE_RPC_REQUEST_DURATION_SECONDS: &'static str =
        "managed_mode_rpc_request_duration_seconds";

    // --- RPC Method Names (for zeroing) ---
    /// Lists all managed mode RPC methods here to ensure they are pre-registered.
    const RPC_METHODS: [&'static str; 5] = [
        "fetch_receipts",
        "output_v0_at_timestamp",
        "pending_output_v0_at_timestamp",
        "l2_block_ref_by_timestamp",
        "provide_l1",
    ];

    /// Initializes metrics for the Supervisor RPC service.
    ///
    /// This does two things:
    /// * Describes various metrics.
    /// * Initializes metrics with their labels to 0 so they can be queried immediately.
    pub(crate) fn init() {
        Self::describe();
        Self::zero();
    }

    /// Describes metrics used in the Supervisor RPC service.
    fn describe() {
        metrics::describe_counter!(
            Self::MANAGED_MODE_RPC_REQUESTS_SUCCESS_TOTAL,
            metrics::Unit::Count,
            "Total number of successful RPC requests processed by the managed mode client"
        );
        metrics::describe_counter!(
            Self::MANAGED_MODE_RPC_REQUESTS_ERROR_TOTAL,
            metrics::Unit::Count,
            "Total number of failed RPC requests processed by the managed mode client"
        );
        metrics::describe_histogram!(
            Self::MANAGED_MODE_RPC_REQUEST_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Duration of RPC requests processed by the managed mode client"
        );
    }

    /// Initializes metrics with their labels to `0` so they appear in Prometheus from the start.
    fn zero() {
        for method_name in Self::RPC_METHODS.iter() {
            metrics::counter!(
                Self::MANAGED_MODE_RPC_REQUESTS_SUCCESS_TOTAL,
                "method" => *method_name
            )
            .increment(0);
            metrics::counter!(
                Self::MANAGED_MODE_RPC_REQUESTS_ERROR_TOTAL,
                "method" => *method_name
            )
            .increment(0);
            metrics::histogram!(
                Self::MANAGED_MODE_RPC_REQUEST_DURATION_SECONDS,
                "method" => *method_name
            )
            .record(0.0); // Record a zero value to ensure the label combination is present
        }
    }
}

/// See [`observe_rpc_call`](crate::observe_rpc_call).
#[macro_export]
macro_rules! observe_rpc_call_managed_mode {
    ($method_name:expr, $block:expr) => {{
        let start_time = std::time::Instant::now();
        let result = $block; // Execute the provided code block
        let duration = start_time.elapsed().as_secs_f64();

        if result.is_ok() {
            metrics::counter!($crate::syncnode::metrics::Metrics::MANAGED_MODE_RPC_REQUESTS_SUCCESS_TOTAL, "method" => $method_name).increment(1);
        } else {
            metrics::counter!($crate::syncnode::metrics::Metrics::MANAGED_MODE_RPC_REQUESTS_ERROR_TOTAL, "method" => $method_name).increment(1);
        }

        metrics::histogram!($crate::syncnode::metrics::Metrics::MANAGED_MODE_RPC_REQUEST_DURATION_SECONDS, "method" => $method_name).record(duration);
        result
    }};
}
