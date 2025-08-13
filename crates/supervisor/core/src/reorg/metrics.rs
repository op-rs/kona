use alloy_primitives::ChainId;

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

    pub(crate) fn record_block_depth(chain_id: ChainId, l1_depth: u64, l2_depth: u64) {
        metrics::histogram!(
            Self::SUPERVISOR_REORG_L1_DEPTH,
            "chain_id" => chain_id.to_string(),
        )
        .record(l1_depth as f64);

        metrics::histogram!(
            Self::SUPERVISOR_REORG_L2_DEPTH,
            "chain_id" => chain_id.to_string(),
        )
        .record(l2_depth as f64);
    }
}
