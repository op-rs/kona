use crate::ChainProcessorError;
use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use std::time::SystemTime;
use tracing::error;

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

    pub(crate) const BLOCK_TYPE_LOCAL_UNSAFE: &'static str = "local_unsafe";
    pub(crate) const BLOCK_TYPE_CROSS_UNSAFE: &'static str = "cross_unsafe";
    pub(crate) const BLOCK_TYPE_LOCAL_SAFE: &'static str = "local_safe";
    pub(crate) const BLOCK_TYPE_CROSS_SAFE: &'static str = "cross_safe";
    pub(crate) const BLOCK_TYPE_FINALIZED: &'static str = "finalized";

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

    fn zero_block_processing(chain_id: ChainId, block_type: &'static str) {
        metrics::counter!(
            Self::BLOCK_PROCESSING_SUCCESS_TOTAL,
            "type" => block_type,
            "chain_id" => chain_id.to_string()
        )
        .increment(0);

        metrics::counter!(
            Self::BLOCK_PROCESSING_ERROR_TOTAL,
            "type" => block_type,
            "chain_id" => chain_id.to_string()
        )
        .increment(0);

        metrics::histogram!(
            Self::BLOCK_PROCESSING_LATENCY_SECONDS,
            "type" => block_type,
            "chain_id" => chain_id.to_string()
        )
        .record(0.0);
    }

    fn zero(chain_id: ChainId) {
        Self::zero_block_processing(chain_id, Self::BLOCK_TYPE_LOCAL_UNSAFE);
        Self::zero_block_processing(chain_id, Self::BLOCK_TYPE_CROSS_UNSAFE);
        Self::zero_block_processing(chain_id, Self::BLOCK_TYPE_LOCAL_SAFE);
        Self::zero_block_processing(chain_id, Self::BLOCK_TYPE_CROSS_SAFE);
        Self::zero_block_processing(chain_id, Self::BLOCK_TYPE_FINALIZED);
    }

    /// Records metrics for a block processing operation.
    /// Takes the result of the processing and extracts the block info if successful.
    pub(crate) fn record_block_processing(
        chain_id: ChainId,
        block_type: &'static str,
        block: BlockInfo,
        result: &Result<(), ChainProcessorError>,
    ) {
        match result {
            Ok(_) => {
                metrics::counter!(
                    Self::BLOCK_PROCESSING_SUCCESS_TOTAL,
                    "type" => block_type,
                    "chain_id" => chain_id.to_string()
                )
                .increment(1);

                // Calculate latency
                match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => {
                        let now = duration.as_secs_f64();
                        let latency = now - block.timestamp as f64;

                        metrics::histogram!(
                            Self::BLOCK_PROCESSING_LATENCY_SECONDS,
                            "type" => block_type,
                            "chain_id" => chain_id.to_string()
                        )
                        .record(latency);
                    }
                    Err(err) => {
                        error!(
                            target: "chain_processor",
                            chain_id = chain_id,
                            %err,
                            "SystemTime error when recording block processing latency"
                        );
                    }
                }
            }
            Err(_) => {
                metrics::counter!(
                    Self::BLOCK_PROCESSING_ERROR_TOTAL,
                    "type" => block_type,
                    "chain_id" => chain_id.to_string()
                )
                .increment(1);
            }
        }
    }
}
