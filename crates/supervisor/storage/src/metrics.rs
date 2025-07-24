use alloy_primitives::ChainId;

/// Container for ChainDb metrics.
#[derive(Debug, Clone)]
pub(crate) struct Metrics;

// todo: implement this using the reth metrics for tables
impl Metrics {
    pub(crate) const STORAGE_REQUESTS_SUCCESS_TOTAL: &'static str =
        "kona_supervisor_storage_success_total";
    pub(crate) const STORAGE_REQUESTS_ERROR_TOTAL: &'static str =
        "kona_supervisor_storage_error_total";
    pub(crate) const STORAGE_REQUEST_DURATION_SECONDS: &'static str =
        "kona_supervisor_storage_duration_seconds";

    // List all your ChainDb method names here
    const METHODS: [&'static str; 21] = [
        "derived_to_source",
        "latest_derived_block_at_source",
        "latest_derivation_state",
        "initialise_derivation_storage",
        "save_derived_block",
        "save_source_block",
        "get_latest_block",
        "get_block",
        "get_log",
        "get_logs",
        "initialise_log_storage",
        "store_block_logs",
        "get_safety_head_ref",
        "get_super_head",
        "update_finalized_using_source",
        "update_current_cross_unsafe",
        "update_current_cross_safe",
        "update_finalized_l1",
        "get_finalized_l1",
        "rewind_log_storage",
        "rewind", // Add more as needed
    ];

    pub(crate) fn init(chain_id: ChainId) {
        Self::describe();
        Self::zero(chain_id);
    }

    fn describe() {
        metrics::describe_counter!(
            Self::STORAGE_REQUESTS_SUCCESS_TOTAL,
            metrics::Unit::Count,
            "Total number of successful Kona Supervisor Storage requests"
        );
        metrics::describe_counter!(
            Self::STORAGE_REQUESTS_ERROR_TOTAL,
            metrics::Unit::Count,
            "Total number of failed Kona Supervisor Storage requests"
        );
        metrics::describe_histogram!(
            Self::STORAGE_REQUEST_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Duration of Kona Supervisor Storage requests"
        );
    }

    fn zero(chain_id: ChainId) {
        for method_name in Self::METHODS.iter() {
            metrics::counter!(
                Self::STORAGE_REQUESTS_SUCCESS_TOTAL,
                "method" => *method_name,
                "chain_id" => chain_id.to_string()
            )
            .increment(0);
            metrics::counter!(
                Self::STORAGE_REQUESTS_ERROR_TOTAL,
                "method" => *method_name,
                "chain_id" => chain_id.to_string()
            )
            .increment(0);
            metrics::histogram!(
                Self::STORAGE_REQUEST_DURATION_SECONDS,
                "method" => *method_name,
                "chain_id" => chain_id.to_string()
            )
            .record(0.0);
        }
    }
}
