use crate::SupervisorError;
use alloy_primitives::ChainId;
use std::time::Instant;

#[derive(Debug, Clone)]
pub(crate) struct ReorgDepth {
    pub(crate) l1_depth: u64,
    pub(crate) l2_depth: u64,
}

/// Metrics for reorg operations
#[derive(Debug, Clone)]
pub(crate) struct Metrics;

impl Metrics {
    pub(crate) const SUPERVISOR_REORG_SUCCESS: &'static str = "kona_supervisor_reorg_success";
    pub(crate) const SUPERVISOR_REORG_ERROR: &'static str = "kona_supervisor_reorg_error";
    pub(crate) const SUPERVISOR_REORG_DURATION_SECONDS: &'static str =
        "kona_supervisor_reorg_duration_seconds";
    pub(crate) const SUPERVISOR_REORG_L1_DEPTH: &'static str = "kona_supervisor_reorg_l1_depth";
    pub(crate) const SUPERVISOR_REORG_L2_DEPTH: &'static str = "kona_supervisor_reorg_l2_depth";

    pub(crate) fn init(chain_id: ChainId) {
        Self::describe();
        Self::zero(chain_id);
    }

    fn describe() {
        metrics::describe_counter!(
            Self::SUPERVISOR_REORG_SUCCESS,
            metrics::Unit::Count,
            "Total number of successfully processed L1 reorgs in the supervisor",
        );

        metrics::describe_counter!(
            Self::SUPERVISOR_REORG_ERROR,
            metrics::Unit::Count,
            "Total number of errors encountered while processing L1 reorgs in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_REORG_L1_DEPTH,
            metrics::Unit::Count,
            "Depth of the L1 reorg in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_REORG_L2_DEPTH,
            metrics::Unit::Count,
            "Depth of the L2 reorg in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_REORG_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Latency for processing L1 reorgs in the supervisor",
        );
    }

    fn zero(chain_id: ChainId) {
        metrics::counter!(Self::SUPERVISOR_REORG_SUCCESS, "chain_id" => chain_id.to_string())
            .increment(0);

        metrics::counter!(Self::SUPERVISOR_REORG_ERROR, "chain_id" => chain_id.to_string())
            .increment(0);

        metrics::histogram!(Self::SUPERVISOR_REORG_L1_DEPTH, "chain_id" => chain_id.to_string())
            .record(0);

        metrics::histogram!(Self::SUPERVISOR_REORG_L2_DEPTH, "chain_id" => chain_id.to_string())
            .record(0);

        metrics::histogram!(Self::SUPERVISOR_REORG_DURATION_SECONDS, "chain_id" => chain_id.to_string())
            .record(0.0);
    }

    /// Records metrics for a L1 reorg processing operation.
    /// Takes the result of the processing and extracts the reorg depth if successful.
    pub(crate) fn record_l1_reorg_processing(
        chain_id: ChainId,
        start_time: Instant,
        result: &Result<ReorgDepth, SupervisorError>,
    ) {
        match result {
            Ok(reorg_depth) => {
                metrics::counter!(
                    Self::SUPERVISOR_REORG_SUCCESS,
                    "chain_id" => chain_id.to_string(),
                )
                .increment(1);

                metrics::histogram!(
                    Self::SUPERVISOR_REORG_L1_DEPTH,
                    "chain_id" => chain_id.to_string(),
                )
                .record(reorg_depth.l1_depth as f64);

                metrics::histogram!(
                    Self::SUPERVISOR_REORG_L2_DEPTH,
                    "chain_id" => chain_id.to_string(),
                )
                .record(reorg_depth.l2_depth as f64);

                // Calculate latency
                let latency = start_time.elapsed().as_secs_f64();

                metrics::histogram!(
                    Self::SUPERVISOR_REORG_DURATION_SECONDS,
                    "chain_id" => chain_id.to_string(),
                )
                .record(latency);
            }
            Err(_) => {
                metrics::counter!(
                    Self::SUPERVISOR_REORG_ERROR,
                    "chain_id" => chain_id.to_string(),
                )
                .increment(1);
            }
        }
    }
}
