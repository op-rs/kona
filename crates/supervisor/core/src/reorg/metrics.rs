use crate::SupervisorError;
use alloy_primitives::ChainId;
use std::time::SystemTime;
use tracing::error;

#[derive(Debug, Clone)]
pub(crate) struct Metrics;

impl Metrics {
    pub(crate) const SUPERVISOR_L1_REORG_SUCCESS_TOTAL: &'static str =
        "kona_supervisor_l1_reorg_success_total";
    pub(crate) const SUPERVISOR_L1_REORG_ERROR_TOTAL: &'static str =
        "kona_supervisor_l1_reorg_error_total";
    pub(crate) const SUPERVISOR_L1_REORG_DURATION_SECONDS: &'static str =
        "kona_supervisor_l1_reorg_duration_seconds";
    pub(crate) const SUPERVISOR_L1_REORG_L1_DEPTH: &'static str =
        "kona_supervisor_l1_reorg_l1_depth";
    pub(crate) const SUPERVISOR_L1_REORG_L2_DEPTH: &'static str =
        "kona_supervisor_l1_reorg_l2_depth";

    pub(crate) fn init() {
        Self::describe();
        Self::zero();
    }

    fn describe() {
        metrics::describe_counter!(
            Self::SUPERVISOR_L1_REORG_SUCCESS_TOTAL,
            metrics::Unit::Count,
            "Total number of successfully processed L1 reorgs in the supervisor",
        );

        metrics::describe_counter!(
            Self::SUPERVISOR_L1_REORG_ERROR_TOTAL,
            metrics::Unit::Count,
            "Total number of errors encountered while processing L1 reorgs in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_L1_REORG_L1_DEPTH,
            metrics::Unit::Count,
            "Depth of the L1 reorg in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_L1_REORG_L2_DEPTH,
            metrics::Unit::Count,
            "Depth of the L2 reorg in the supervisor",
        );

        metrics::describe_histogram!(
            Self::SUPERVISOR_L1_REORG_DURATION_SECONDS,
            metrics::Unit::Seconds,
            "Latency for processing L1 reorgs in the supervisor",
        );
    }

    fn zero() {
        metrics::counter!(Self::SUPERVISOR_L1_REORG_SUCCESS_TOTAL,).increment(0);

        metrics::counter!(Self::SUPERVISOR_L1_REORG_ERROR_TOTAL,).increment(0);

        metrics::histogram!(Self::SUPERVISOR_L1_REORG_L1_DEPTH,).record(0);

        metrics::histogram!(Self::SUPERVISOR_L1_REORG_L2_DEPTH,).record(0);

        metrics::histogram!(Self::SUPERVISOR_L1_REORG_DURATION_SECONDS,).record(0.0);
    }

    pub(crate) fn record_l1_reorg_processing(
        chain_id: ChainId,
        start_time: f64,
        result: &Result<(u64, u64), SupervisorError>,
    ) {
        match result {
            Ok((l1_depth, l2_depth)) => {
                metrics::counter!(
                    Self::SUPERVISOR_L1_REORG_SUCCESS_TOTAL,
                    "chain_id" => chain_id.to_string(),
                )
                .increment(1);

                metrics::histogram!(
                    Self::SUPERVISOR_L1_REORG_L1_DEPTH,
                    "chain_id" => chain_id.to_string(),
                )
                .record(*l1_depth as f64);

                metrics::histogram!(
                    Self::SUPERVISOR_L1_REORG_L2_DEPTH,
                    "chain_id" => chain_id.to_string(),
                )
                .record(*l2_depth as f64);

                // Calculate latency
                match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => {
                        let now = duration.as_secs_f64();
                        let latency = now - start_time;

                        metrics::histogram!(
                            Self::SUPERVISOR_L1_REORG_DURATION_SECONDS,
                            "chain_id" => chain_id.to_string(),
                        )
                        .record(latency);
                    }
                    Err(err) => {
                        error!(
                            target: "supervisor::reorg_handler",
                            chain_id = chain_id,
                            %err,
                            "Failed to get current time for reorg latency"
                        );
                    }
                }
            }
            Err(_) => {
                metrics::counter!(
                    Self::SUPERVISOR_L1_REORG_ERROR_TOTAL,
                    "chain_id" => chain_id.to_string(),
                )
                .increment(1);
            }
        }
    }
}
