//! A task to insert an unsafe payload into the execution engine.

use crate::{
    EngineClient, EngineForkchoiceVersion, EngineState, EngineTaskError, EngineTaskExt,
    InsertUnsafeTaskError, SyncConfig, SyncMode, SyncStatus,
};
use alloy_eips::BlockNumberOrTag;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{
    ExecutionPayloadInputV2, ForkchoiceState, INVALID_FORK_CHOICE_STATE_ERROR, PayloadStatusEnum,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpBlock;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{OpExecutionPayload, OpNetworkPayloadEnvelope};
use std::{sync::Arc, time::Instant};

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug, Clone)]
pub struct InsertUnsafeTask {
    /// The engine client.
    client: Arc<EngineClient>,
    /// The sync config.
    sync_config: Arc<SyncConfig>,
    /// The rollup config.
    rollup_config: Arc<RollupConfig>,
    /// The engine forkchoice version.
    version: EngineForkchoiceVersion,
    /// The network payload envelope.
    envelope: OpNetworkPayloadEnvelope,
}

impl InsertUnsafeTask {
    /// Creates a new insert task.
    pub fn new(
        client: Arc<EngineClient>,
        sync_config: Arc<SyncConfig>,
        rollup_config: Arc<RollupConfig>,
        envelope: OpNetworkPayloadEnvelope,
    ) -> Self {
        let version =
            EngineForkchoiceVersion::from_cfg(rollup_config.as_ref(), envelope.payload.timestamp());
        Self { client, sync_config, rollup_config, version, envelope }
    }

    /// Checks the response of the `engine_newPayload` call, and updates the sync status if
    /// necessary.
    fn check_new_payload_status(
        &self,
        state: &mut EngineState,
        status: &PayloadStatusEnum,
    ) -> bool {
        if self.sync_config.sync_mode == SyncMode::ExecutionLayer {
            debug!(target: "engine", ?status, "Checking payload status");
            if matches!(status, PayloadStatusEnum::Valid) {
                debug!(target: "engine", "Valid new payload status. Finished execution layer sync");
                state.sync_status = SyncStatus::ExecutionLayerFinished;
            }
            return matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing);
        }

        matches!(status, PayloadStatusEnum::Valid)
    }

    /// Checks the response of the `engine_forkchoiceUpdated` call, and updates the sync status if
    /// necessary.
    fn check_forkchoice_updated_status(
        &self,
        state: &mut EngineState,
        status: &PayloadStatusEnum,
    ) -> bool {
        if self.sync_config.sync_mode == SyncMode::ExecutionLayer {
            if matches!(status, PayloadStatusEnum::Valid) &&
                state.sync_status == SyncStatus::ExecutionLayerStarted
            {
                state.sync_status = SyncStatus::ExecutionLayerNotFinalized;
            }

            return matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing);
        }

        matches!(status, PayloadStatusEnum::Valid)
    }
}

#[async_trait]
impl EngineTaskExt for InsertUnsafeTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // If there is a finalized head block, transition to EL sync.
        if state.sync_status == SyncStatus::ExecutionLayerWillStart {
            debug!(target: "engine", "Checking finalized block");
            let finalized_block =
                self.client.l2_block_info_by_label(BlockNumberOrTag::Finalized).await;
            match finalized_block {
                Ok(finalized_block) => {
                    let finalized_genesis = finalized_block
                        .is_some_and(|b| b.block_info.hash == self.rollup_config.genesis.l2.hash);

                    if finalized_block.is_none() ||
                        finalized_genesis ||
                        self.sync_config.supports_post_finalization_elsync
                    {
                        info!(target: "engine", "Starting execution layer sync");
                        state.sync_status = SyncStatus::ExecutionLayerStarted;
                    } else if finalized_block.is_some() {
                        info!(target: "engine", "Found finalized block; Skipping EL sync and starting CL sync.");
                        state.sync_status = SyncStatus::ExecutionLayerFinished;
                    }
                }
                Err(_) => return Err(InsertUnsafeTaskError::FinalizedBlockFetch.into()),
            }
        }

        let time_start = Instant::now();

        // Insert the new payload.
        let block_root = self.envelope.parent_beacon_block_root.unwrap_or_default();
        let insert_time_start = Instant::now();
        let response = match self.envelope.payload.clone() {
            OpExecutionPayload::V1(payload) => self.client.new_payload_v1(payload).await,
            OpExecutionPayload::V2(payload) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: payload.payload_inner,
                    withdrawals: Some(payload.withdrawals),
                };
                self.client.new_payload_v2(payload_input).await
            }
            OpExecutionPayload::V3(payload) => {
                self.client.new_payload_v3(payload, block_root).await
            }
            OpExecutionPayload::V4(payload) => {
                self.client.new_payload_v4(payload, block_root).await
            }
        };

        // Check the `engine_newPayload` response.
        let response = match response {
            Ok(resp) => resp,
            Err(e) => return Err(InsertUnsafeTaskError::InsertFailed(e).into()),
        };
        if !self.check_new_payload_status(state, &response.status) {
            return Err(InsertUnsafeTaskError::UnexpectedPayloadStatus(response.status).into());
        }
        let insert_duration = insert_time_start.elapsed();

        // Form the new unsafe block ref from the execution payload.
        let block: OpBlock = self
            .envelope
            .payload
            .clone()
            .try_into_block()
            .map_err(InsertUnsafeTaskError::FromBlockError)?;
        let new_unsafe_ref =
            L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
                .map_err(InsertUnsafeTaskError::L2BlockInfoConstruction)?;

        let mut fcu = ForkchoiceState {
            head_block_hash: self.envelope.payload.block_hash(),
            safe_block_hash: state.safe_head().block_info.hash,
            finalized_block_hash: state.finalized_head().block_info.hash,
        };
        if state.sync_status == SyncStatus::ExecutionLayerNotFinalized {
            // Use the new payload as the safe and finalized block for the FCU.
            fcu.safe_block_hash = self.envelope.payload.block_hash();
            fcu.finalized_block_hash = self.envelope.payload.block_hash();

            // Update the local engine state to match.
            state.set_unsafe_head(new_unsafe_ref);
            state.set_safe_head(new_unsafe_ref);
            state.set_local_safe_head(new_unsafe_ref);
            state.set_finalized_head(new_unsafe_ref);
        }

        // Send the forkchoice update to finalize the payload insertion.
        let fcu_time_start = Instant::now();
        let response = match self.version {
            EngineForkchoiceVersion::V1 => self.client.fork_choice_updated_v1(fcu, None).await,
            EngineForkchoiceVersion::V2 => self.client.fork_choice_updated_v2(fcu, None).await,
            EngineForkchoiceVersion::V3 => self.client.fork_choice_updated_v3(fcu, None).await,
        };
        let fcu_duration = fcu_time_start.elapsed();
        let total_duration = time_start.elapsed();

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                // Check if the error is due to an inconsistent forkchoice state. If so, we need to
                // signal for a pipeline reset.
                let e = e
                    .as_error_resp()
                    .and_then(|err| {
                        (err.code == INVALID_FORK_CHOICE_STATE_ERROR as i64)
                            .then_some(InsertUnsafeTaskError::InconsistentForkchoiceState)
                    })
                    .unwrap_or(InsertUnsafeTaskError::ForkchoiceUpdateFailed(e));
                return Err(e.into());
            }
        };
        if !self.check_forkchoice_updated_status(state, &response.payload_status.status) {
            return Err(InsertUnsafeTaskError::UnexpectedPayloadStatus(
                response.payload_status.status,
            )
            .into());
        }

        // Update the local engine state.
        state.set_unsafe_head(new_unsafe_ref);
        state.forkchoice_update_needed = false;

        // Finish EL sync if the EL sync state was finished, but not finalized before this
        // operation.
        if state.sync_status == SyncStatus::ExecutionLayerNotFinalized {
            info!(
                target: "engine",
                finalized_block = new_unsafe_ref.block_info.number,
                "Finished execution layer sync."
            );
            state.sync_status = SyncStatus::ExecutionLayerFinished;
        }

        info!(
            target: "engine",
            hash = new_unsafe_ref.block_info.hash.to_string(),
            number = new_unsafe_ref.block_info.number,
            total_duration = ?total_duration,
            insert_duration = ?insert_duration,
            fcu_duration = ?fcu_duration,
            "Inserted new unsafe block"
        );
        Ok(())
    }
}
