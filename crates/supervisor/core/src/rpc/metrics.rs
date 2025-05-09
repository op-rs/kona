//! Metrics for the Supervisor RPC service.

/// Container for metrics.
#[derive(Debug, Clone)]
pub(crate) struct Metrics;

impl Metrics {
    // --- Metric Names ---
    /// Identifier for the counter of successful RPC requests. Labels: `method`.
    pub(crate) const SUPERVISOR_RPC_REQUESTS_SUCCESS_TOTAL: &'static str =
        "supervisor_rpc_requests_success_total";
    /// Identifier for the counter of failed RPC requests. Labels: `method`.
    pub(crate) const SUPERVISOR_RPC_REQUESTS_ERROR_TOTAL: &'static str =
        "supervisor_rpc_requests_error_total";
    /// Identifier for the histogram of RPC request durations. Labels: `method`.
    pub(crate) const SUPERVISOR_RPC_REQUEST_DURATION_SECONDS: &'static str =
        "supervisor_rpc_request_duration_seconds";

    // --- RPC Method Names (for zeroing) ---
    // List all your supervisor RPC methods here to ensure they are pre-registered.
    const RPC_METHODS: [&'static str; 9] = [
        "cross_derived_to_source",
        "local_unsafe",
        "cross_safe",
        "finalized",
        "finalized_l1",
        "super_root_at_timestamp",
        "sync_status",
        "all_safe_derived_at",
        "check_access_list",
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
            Self::SUPERVISOR_RPC_REQUESTS_SUCCESS_TOTAL,
            metrics::Unit::Count,
            "Total number of successful RPC requests processed by the supervisor"
        );
        metrics::describe_counter!(
            Self::SUPERVISOR_RPC_REQUESTS_ERROR_TOTAL,
            metrics::Unit::Count,
            "Total number of failed RPC requests processed by the supervisor"
        );
        metrics::describe_histogram!(
            Self::SUPERVISOR_RPC_REQUEST_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Duration of RPC requests processed by the supervisor"
        );
    }

    /// Initializes metrics with their labels to `0` so they appear in Prometheus from the start.
    fn zero() {
        for method_name in Self::RPC_METHODS.iter() {
            metrics::counter!(
                Self::SUPERVISOR_RPC_REQUESTS_SUCCESS_TOTAL,
                "method" => *method_name
            )
            .increment(0);
            metrics::counter!(
                Self::SUPERVISOR_RPC_REQUESTS_ERROR_TOTAL,
                "method" => *method_name
            )
            .increment(0);
            metrics::histogram!(
                Self::SUPERVISOR_RPC_REQUEST_DURATION_SECONDS,
                "method" => *method_name
            )
            .record(0.0); // Record a zero value to ensure the label combination is present
        }
    }
}

/// Observes an RPC call, recording its duration and outcome.
///
/// # Usage
/// ```ignore
/// async fn my_rpc_method(&self, arg: u32) -> RpcResult<String> {
///     observe_rpc_call!("my_rpc_method_name", {
///         // todo: add actual RPC logic
///         if arg == 0 { Ok("success".to_string()) } else { Err(ErrorObject::owned(1, "failure", None::<()>)) }
///     })
/// }
/// ```
#[macro_export]
macro_rules! observe_rpc_call {
    ($method_name:expr, $block:expr) => {{
        let start_time = std::time::Instant::now();
        let result = $block; // Execute the provided code block
        let duration = start_time.elapsed().as_secs_f64();

        if result.is_ok() {
            metrics::counter!($crate::rpc::metrics::Metrics::SUPERVISOR_RPC_REQUESTS_SUCCESS_TOTAL, "method" => $method_name).increment(1);
        } else {
            metrics::counter!($crate::rpc::metrics::Metrics::SUPERVISOR_RPC_REQUESTS_ERROR_TOTAL, "method" => $method_name).increment(1);
        }

        metrics::histogram!($crate::rpc::metrics::Metrics::SUPERVISOR_RPC_REQUEST_DURATION_SECONDS, "method" => $method_name).record(duration);
        result
    }};
}
