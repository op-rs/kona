//! Contains the engine controller.
//!
//! See: <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46>

use alloy_rpc_types_engine::payload::{PayloadStatus, PayloadStatusEnum};
use kona_genesis::RollupConfig;
use kona_rpc::L2BlockRef;

use crate::{
    engine::{EngineClient, EngineState, SyncStatus},
    sync::SyncConfig,
};

/// The engine controller.
#[derive(Debug, Clone)]
pub struct EngineController {
    /// The internal engine client.
    pub client: EngineClient,
    /// The sync status.
    pub sync_status: SyncStatus,
    /// The engine state.
    pub state: EngineState,

    // Below are extracted fields from the `RollupConfig`.
    // Since they don't change during the lifetime of the `EngineController`,
    // we don't need to store a reference to the `RollupConfig`.
    /// Blocktime of the L2 chain
    pub blocktime: u64,
    /// The ecotone timestamp used for fork choice
    pub ecotone_timestamp: Option<u64>,
    /// The canyon timestamp used for fork choice
    pub canyon_timestamp: Option<u64>,
}

impl EngineController {
    /// Creates a new engine controller.
    pub fn new(client: EngineClient, config: &RollupConfig, sync: SyncConfig) -> Self {
        let sync_status = SyncStatus::from(sync.sync_mode);
        Self {
            client,
            sync_status,
            state: EngineState::new(L2BlockRef {
                l1_block_info: Default::default(),
                l1_origin: Default::default(),
                sequence_number: 0,
            }),
            blocktime: config.block_time,
            ecotone_timestamp: config.hardforks.ecotone_time,
            canyon_timestamp: config.hardforks.canyon_time,
        }
    }

    /// Checks if the payload status is acceptable.
    ///
    /// If the consensus node is currently syncing via execution layer sync,
    /// and the payload is valid, ensure the sync status is updated to finalized.
    ///
    /// The payload status is only acceptable for consensus layer sync if it is valid.
    pub fn check_payload_status(&mut self, status: PayloadStatus) -> bool {
        if self.sync_status == SyncStatus::ConsensusLayer {
            return status.status.is_valid();
        }
        if status.status.is_valid() && self.sync_status.has_started() {
            self.sync_status = SyncStatus::ExecutionLayerNotFinalized;
        }
        status.status.is_valid() ||
            status.status.is_syncing() ||
            status.status == PayloadStatusEnum::Accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::SyncMode;
    use alloy_rpc_types_engine::JwtSecret;
    use std::sync::Arc;

    fn test_controller(sync_mode: SyncMode) -> EngineController {
        let rollup_config = RollupConfig { block_time: 0, ..Default::default() };
        let sync_config = SyncConfig {
            sync_mode,
            skip_sync_start_check: false,
            supports_post_finalization_elsync: false,
        };
        let engine_url: url::Url = "http://localhost:8080".parse().unwrap();
        let rpc_url: url::Url = "http://localhost:8080".parse().unwrap();

        let rollup_config = Arc::new(rollup_config);
        let client = EngineClient::new_http(
            engine_url,
            rpc_url,
            Arc::clone(&rollup_config),
            JwtSecret::random(),
        );
        EngineController::new(client, &rollup_config, sync_config)
    }

    #[test]
    fn test_check_payload_status_cl_sync() {
        let mut controller = test_controller(SyncMode::ConsensusLayer);

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Syncing, latest_valid_hash: None };
        assert!(!controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Accepted, latest_valid_hash: None };
        assert!(!controller.check_payload_status(status));
    }

    #[test]
    fn test_check_payload_status_el_sync() {
        let mut controller = test_controller(SyncMode::ExecutionLayer);
        assert_eq!(controller.sync_status, SyncStatus::ExecutionLayerWillStart);

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Syncing, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Accepted, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));

        let status = PayloadStatus {
            status: PayloadStatusEnum::Invalid { validation_error: Default::default() },
            latest_valid_hash: None,
        };
        assert!(!controller.check_payload_status(status));

        assert_eq!(controller.sync_status, SyncStatus::ExecutionLayerWillStart);
        controller.sync_status = SyncStatus::ExecutionLayerStarted;

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));
        assert_eq!(controller.sync_status, SyncStatus::ExecutionLayerNotFinalized);
    }
}
