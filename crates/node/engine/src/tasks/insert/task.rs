//! A task to insert an unsafe payload into the execution engine.

use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{ExecutionPayload, ExecutionPayloadInputV2, ForkchoiceState};
use kona_protocol::L2BlockInfo;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::sync::Arc;

use crate::{
    EngineClient, EngineForkchoiceVersion, EngineState, EngineTask, InsertTaskError,
    InsertTaskInput, InsertTaskOut, SyncConfig, SyncStatus,
};

/// The input type for the [InsertTask].
/// The third argument is the genesis l2 hash expected to be provided by the rollup config.
type Input = (
    Arc<EngineClient>,
    Arc<SyncConfig>,
    B256,
    EngineForkchoiceVersion,
    OpNetworkPayloadEnvelope,
    L2BlockInfo,
);

/// An external handle to communicate with a [InsertTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct InsertTaskExt {
    /// A receiver channel to receive [InsertTaskOut] events *from* the [InsertTask].
    pub receiver: tokio::sync::broadcast::Receiver<InsertTaskOut>,
    /// A sender channel to send [InsertTaskInput] event *to* the [InsertTask].
    pub sender: tokio::sync::broadcast::Sender<InsertTaskInput>,
    /// A join handle to the spawned thread containing the [InsertTask].
    pub handle: tokio::task::JoinHandle<Result<(), InsertTaskError>>,
}

impl InsertTaskExt {
    /// Spawns the [InsertTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(input: Input) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let mut task = InsertTask::new(task_receiver, task_sender);
        let handle = tokio::spawn(async move { task.execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// A strongly typed receiver channel.
type Receiver = tokio::sync::broadcast::Receiver<InsertTaskInput>;

/// A strongly typed sender channel.
type Sender = tokio::sync::broadcast::Sender<InsertTaskOut>;

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: Receiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: Sender,
}

impl InsertTask {
    /// Creates a new insert task.
    pub const fn new(receiver: Receiver, sender: Sender) -> Self {
        Self { receiver, sender }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, InsertTaskError> {
        crate::send_until_success!("insert", self.sender, InsertTaskOut::SyncStatus);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(InsertTaskError::InvalidSyncStatusResponse)
        }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(&mut self) -> Result<EngineState, InsertTaskError> {
        crate::send_until_success!("insert", self.sender, InsertTaskOut::StateSnapshot);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }

    /// Requests an [L2BlockInfo] for the [BlockNumberOrTag::Finalized].
    pub async fn fetch_finalized_info(&mut self) -> Result<Option<L2BlockInfo>, InsertTaskError> {
        let msg = InsertTaskOut::L2BlockInfo(BlockNumberOrTag::Finalized);
        crate::send_until_success!("insert", self.sender, msg);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::L2BlockInfoResponse(bi) = response {
            Ok(bi)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }
}

#[async_trait::async_trait]
impl EngineTask for InsertTask {
    type Error = InsertTaskError;
    type Input = Input;

    async fn execute(&mut self, input: Self::Input) -> Result<(), Self::Error> {
        let (client, config, genesis, version, envelope, block_info) = input;

        let sync = self.fetch_sync_status().await?;
        // If post finalization EL sync, jump straight to EL sync start
        if config.supports_post_finalization_elsync {
            let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
            crate::send_until_success!("insert", self.sender, msg);
        } else if sync == SyncStatus::ExecutionLayerWillStart {
            match self.fetch_finalized_info().await {
                Ok(Some(bi)) => {
                    // If the genesis is finalized, start EL
                    if bi.block_info.hash == genesis {
                        let msg =
                            InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
                        crate::send_until_success!("insert", self.sender, msg);
                    } else {
                        // If there is a finalized block, finish EL sync.
                        let msg =
                            InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
                        crate::send_until_success!("insert", self.sender, msg);
                    }
                }
                Ok(None) => {
                    // If no block is found, EL started
                    let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerStarted);
                    crate::send_until_success!("insert", self.sender, msg);
                }
                Err(_) => {
                    // Temporary derivation error.
                    return Err(InsertTaskError::TemporaryDerivationError);
                }
            }
        }

        let block_root = envelope.parent_beacon_block_root.unwrap_or_default();
        let response = match envelope.payload.clone() {
            ExecutionPayload::V1(payload) => client.new_payload_v1(payload).await,
            ExecutionPayload::V2(payload) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: payload.payload_inner,
                    withdrawals: Some(payload.withdrawals),
                };
                client.new_payload_v2(payload_input).await
            }
            ExecutionPayload::V3(payload) => client.new_payload_v3(payload, block_root).await,
        };
        let status = match response {
            Ok(s) => s,
            Err(_) => {
                return Err(InsertTaskError::FailedToInsertNewPayload);
            }
        };

        if status.is_invalid() {
            let msg = InsertTaskOut::InvalidNewPayload;
            crate::send_until_success!("insert", self.sender, msg);
        }

        // Tell the engine state to update if needed with the new payload.
        let msg = InsertTaskOut::UpdatePayloadStatus(status);
        crate::send_until_success!("insert", self.sender, msg);

        // Receive response from state.
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        match response {
            InsertTaskInput::NewPayloadResponse(true) => {
                info!(target: "insert", "Engine State updated with new payload.");
            }
            InsertTaskInput::NewPayloadResponse(false) => {
                warn!(target: "insert", "Failed to update Engine State with new payload.");
                let msg = InsertTaskOut::FailedToProcessNewPayload;
                crate::send_until_success!("insert", self.sender, msg);
            }
            _ => {
                return Err(InsertTaskError::InvalidMessageResponse);
            }
        }

        // Fetch the state snapshot.
        let state = self.fetch_state().await?;

        // Mark the new payload as valid
        let mut fcu = ForkchoiceState {
            head_block_hash: block_info.block_info.hash,
            safe_block_hash: state.safe_head.block_info.hash,
            finalized_block_hash: state.finalized_head.block_info.hash,
        };

        // Fetch the sync status
        // SEE: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L407-L416
        let sync = self.fetch_sync_status().await?;
        if sync == SyncStatus::ExecutionLayerNotFinalized {
            fcu.safe_block_hash = envelope.payload.block_hash();
            fcu.finalized_block_hash = envelope.payload.block_hash();

            // Tell the safe to update the heads given the new payload l2 block ref.
            let msg = InsertTaskOut::NewPayloadNotFinalizedUpdate(block_info);
            crate::send_until_success!("insert", self.sender, msg);

            // Broadcast cross safe update event.
            let msg = InsertTaskOut::CrossSafeUpdate(block_info);
            crate::send_until_success!("insert", self.sender, msg);
        }

        // Perform the fork choice update.
        let response = match version {
            EngineForkchoiceVersion::V1 => client.fork_choice_updated_v1(fcu, None).await,
            EngineForkchoiceVersion::V2 => client.fork_choice_updated_v2(fcu, None).await,
            EngineForkchoiceVersion::V3 => client.fork_choice_updated_v3(fcu, None).await,
        };

        let update = match response {
            Ok(u) => u,
            Err(_) => {
                // TODO: reset error for invalid forkchoice state.
                // Otherwise, temporary derivation error.
                return Err(InsertTaskError::TemporaryDerivationError);
            }
        };

        // Send a forkchoice updated status to the engine state.
        // TODO: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L434-L441
        let msg = InsertTaskOut::UpdateForkchoiceState(update.payload_status.clone(), block_info);
        crate::send_until_success!("insert", self.sender, msg);

        // At this point finish EL sync if not finalized.
        let sync = self.fetch_sync_status().await?;
        if sync == SyncStatus::ExecutionLayerNotFinalized {
            let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
            crate::send_until_success!("insert", self.sender, msg);
            // TODO: add duration to the log.
            info!(target: "insert", "Finished EL sync");
        }

        if update.is_valid() {
            let msg = InsertTaskOut::ForkchoiceUpdated;
            crate::send_until_success!("insert", self.sender, msg);
        }

        // TODO: make this log more verbose?
        info!(target: "insert", "Inserted new L2 unsafe block (synchronous)");
        debug!(target: "insert", "(unsafe l2 block) Hash {}", envelope.payload.block_hash());
        debug!(target: "insert", "(unsafe l2 block) Number {}", envelope.payload.block_number());

        Ok(())
    }
}
