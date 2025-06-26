use alloy_primitives::ChainId;

#[derive(Debug)]
pub(crate) struct Metrics;

impl Metrics {
    // --- Metric Names ---
    /// Identifier for block processing success.
    /// Labels: `chain_id`, `type`
    pub(crate) const BLOCK_PROCESSING_SUCCESS_TOTAL: &'static str =
        "supervisor_block_processing_success_total";

    /// Identifier for block processing errors.
    /// Labels: `chain_id`, `type`
    pub(crate) const BLOCK_PROCESSING_ERROR_TOTAL: &'static str =
        "supervisor_block_processing_error_total";

    /// Identifier for block processing latency.
    /// Labels: `chain_id`, `type`
    pub(crate) const BLOCK_PROCESSING_LATENCY_SECONDS: &'static str =
        "supervisor_block_processing_latency_seconds";

    const TYPES: [&'static str; 5] =
        ["local_unsafe", "cross_unsafe", "local_safe", "cross_safe", "finalized"];

    pub(crate) fn init(chain_id: ChainId) {
        Self::describe();
        Self::zero(chain_id);
    }

    fn describe() {
        metrics::describe_counter!(
            Self::BLOCK_PROCESSING_SUCCESS_TOTAL,
            metrics::Unit::Count,
            "Total number of successfully processed blocks in the supervisor",
        );

        metrics::describe_counter!(
            Self::BLOCK_PROCESSING_ERROR_TOTAL,
            metrics::Unit::Count,
            "Total number of errors encountered while processing blocks in the supervisor",
        );

        metrics::describe_histogram!(
            Self::BLOCK_PROCESSING_LATENCY_SECONDS,
            metrics::Unit::Seconds,
            "Latency for processing in the supervisor",
        );
    }

    fn zero(chain_id: ChainId) {
        for &type_name in Self::TYPES.iter() {
            metrics::counter!(
                Self::BLOCK_PROCESSING_SUCCESS_TOTAL,
                "type" => type_name,
                "chain_id" => chain_id.to_string()
            )
            .increment(0);

            metrics::counter!(
                Self::BLOCK_PROCESSING_ERROR_TOTAL,
                "type" => type_name,
                "chain_id" => chain_id.to_string()
            )
            .increment(0);

            metrics::histogram!(
                Self::BLOCK_PROCESSING_LATENCY_SECONDS,
                "type" => type_name,
                "chain_id" => chain_id.to_string()
            )
            .record(0.0);
        }
    }
}
