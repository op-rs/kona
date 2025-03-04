//! Contains the engine controller.
//!
//! See: <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46>

use alloy_network::AnyNetwork;
use alloy_primitives::Bytes;
use alloy_provider::{RootProvider, ext::EngineApi};
use alloy_rpc_types_engine::{
    ForkchoiceState, ForkchoiceUpdated,
    payload::{PayloadStatus, PayloadStatusEnum},
};
use alloy_transport_http::{
    AuthService, Http, HyperClient,
    hyper_util::client::legacy::{Client, connect::HttpConnector},
};

use http_body_util::Full;
use kona_protocol::L2BlockInfo;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpPayloadAttributes;

use crate::{ControllerBuilder, EngineClient, EngineState, EngineUpdateError, SyncStatus};

/// A Hyper HTTP client with a JWT authentication layer.
type HyperAuthClient<B = Full<Bytes>> = HyperClient<B, AuthService<Client<HttpConnector, B>>>;

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
    /// Returns the [`ControllerBuilder`] that is used to construct the [`EngineController`].
    pub const fn builder(client: EngineClient) -> ControllerBuilder {
        ControllerBuilder::new(client)
    }

    /// Returns if the engine is syncing.
    pub const fn is_syncing(&self) -> bool {
        self.sync_status.is_syncing()
    }

    /// Checks if the payload status is acceptable.
    ///
    /// This is the returned status of `engine_newPayloadV1` request
    /// to check for the next payload.
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

    /// Check the returned status of a forkchoice updated request.
    pub fn check_forkchoice_updated_status(&mut self, status: PayloadStatus) -> bool {
        if self.sync_status == SyncStatus::ConsensusLayer {
            return status.status.is_valid();
        }
        if status.status.is_valid() && self.sync_status.has_started() {
            self.sync_status = SyncStatus::ExecutionLayerNotFinalized;
        }
        status.status.is_valid() || status.status.is_syncing()
    }

    /// Calls the right forkchoice updated method based on the timestamp
    async fn forkchoice_update(
        &mut self,
        forkchoice: ForkchoiceState,
        attributes: Option<OpPayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, EngineUpdateError> {
        if attributes.is_none() {
            return <RootProvider<AnyNetwork> as OpEngineApi<
                AnyNetwork,
                Http<HyperAuthClient>,
            >>::fork_choice_updated_v3(&self.client, forkchoice, None)
                .await
                .map_err(EngineUpdateError::from);
        }
        let ts = attributes.as_ref().map_or(0, |a| a.payload_attributes.timestamp);
        let ecotone_active = self.ecotone_timestamp.is_some_and(|e| ts >= e);
        let canyon_active = self.canyon_timestamp.is_some_and(|e| ts >= e);
        if ecotone_active {
            // Cancun
            <RootProvider<AnyNetwork> as OpEngineApi<
                AnyNetwork,
                Http<HyperAuthClient>,
            >>::fork_choice_updated_v3(&self.client, forkchoice, attributes)
                .await
                .map_err(EngineUpdateError::from)
        } else if canyon_active {
            // Shanghai
            <RootProvider<AnyNetwork> as OpEngineApi<
                AnyNetwork,
                Http<HyperAuthClient>,
            >>::fork_choice_updated_v2(&self.client, forkchoice, attributes)
                .await
                .map_err(EngineUpdateError::from)
        } else {
            // According to Ethereum engine API spec, we can use fcuV2 here,
            // but Geth v1.13.11 does not accept V2 before Shanghai.
            self.client
                .fork_choice_updated_v1(forkchoice, attributes.map(|a| a.payload_attributes))
                .await
                .map_err(EngineUpdateError::from)
        }
    }

    /// Attempts to update the engine with the current forkchoice state of the rollup node.
    ///
    /// This is a no-op if the nodes already agree on the forkchoice state.
    pub async fn try_update_engine(&mut self) -> Result<(), EngineUpdateError> {
        if !self.state.forkchoice_update_needed {
            return Err(EngineUpdateError::NoForkchoiceUpdateNeeded);
        }

        if self.is_syncing() {
            tracing::warn!(target: "engine", "Attempting to update forkchoice state while EL syncing");
        }

        if self.state.unsafe_head().block_info.number <
            self.state.finalized_head().block_info.number
        {
            return Err(EngineUpdateError::InvalidForkchoiceState(
                self.state.unsafe_head().block_info.number,
                self.state.finalized_head().block_info.number,
            ));
        }

        let forkchoice = self.state.create_forkchoice_state();
        let update = self.forkchoice_update(forkchoice, None).await?;

        if update.payload_status.is_valid() {
            // TODO: Send a fork choice update message.
        }

        if self.state.unsafe_head() == self.state.safe_head() &&
            self.state.safe_head() == self.state.pending_safe_head()
        {
            self.state.set_backup_unsafe_head(L2BlockInfo::default(), false)
        }
        self.state.forkchoice_update_needed = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rpc_types_engine::JwtSecret;
    use kona_genesis::RollupConfig;
    use std::sync::Arc;

    fn test_provider() -> EngineClient {
        let jwt = JwtSecret::random();
        let cfg = Arc::new(RollupConfig::default());
        let engine: url::Url = "http://localhost:8080".parse().unwrap();
        let rpc: url::Url = "http://localhost:8080".parse().unwrap();

        EngineClient::new_http(engine, rpc, cfg, jwt)
    }

    fn test_controller(sync_status: SyncStatus) -> EngineController {
        let client = test_provider();
        let state = EngineState {
            unsafe_head: L2BlockInfo::default(),
            cross_unsafe_head: L2BlockInfo::default(),
            pending_safe_head: L2BlockInfo::default(),
            local_safe_head: L2BlockInfo::default(),
            safe_head: L2BlockInfo::default(),
            finalized_head: L2BlockInfo::default(),
            backup_unsafe_head: None,
            forkchoice_update_needed: false,
            need_fcu_call_backup_unsafe_reorg: false,
        };
        EngineController {
            client,
            sync_status,
            state,
            blocktime: 0,
            ecotone_timestamp: None,
            canyon_timestamp: None,
        }
    }

    #[test]
    fn test_check_payload_status_cl_sync() {
        let mut controller = test_controller(SyncStatus::ConsensusLayer);

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Syncing, latest_valid_hash: None };
        assert!(!controller.check_payload_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Accepted, latest_valid_hash: None };
        assert!(!controller.check_payload_status(status));
    }

    #[test]
    fn test_check_payload_status_el_sync() {
        let mut controller = test_controller(SyncStatus::ExecutionLayerWillStart);
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

    #[test]
    fn test_check_forkchoice_updated_status_cl_sync() {
        let mut controller = test_controller(SyncStatus::ConsensusLayer);

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_forkchoice_updated_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Syncing, latest_valid_hash: None };
        assert!(!controller.check_forkchoice_updated_status(status));
    }

    #[test]
    fn test_check_forkchoice_updated_status_el_sync() {
        let mut controller = test_controller(SyncStatus::ExecutionLayerWillStart);
        assert_eq!(controller.sync_status, SyncStatus::ExecutionLayerWillStart);

        let status = PayloadStatus { status: PayloadStatusEnum::Valid, latest_valid_hash: None };
        assert!(controller.check_forkchoice_updated_status(status));

        let status = PayloadStatus { status: PayloadStatusEnum::Syncing, latest_valid_hash: None };
        assert!(controller.check_forkchoice_updated_status(status));

        let status = PayloadStatus {
            status: PayloadStatusEnum::Invalid { validation_error: Default::default() },
            latest_valid_hash: None,
        };
        assert!(!controller.check_forkchoice_updated_status(status));
    }
}
