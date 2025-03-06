//! Contains the message types for the insert task.

use crate::{EngineState, SyncStatus};
use alloy_eips::eip1898::BlockNumberOrTag;
use alloy_rpc_types_engine::PayloadStatus;
use kona_protocol::L2BlockInfo;

/// An inbound message from an external actor to the [crate::InsertTask].
#[derive(Debug, Clone)]
pub enum InsertTaskInput {
    /// A response from the sync status request.
    SyncStatusResponse(SyncStatus),
    /// A response from feching L2 block info.
    L2BlockInfoResponse(Option<L2BlockInfo>),
    /// An Engine State new payload response.
    NewPayloadResponse(bool),
    /// A response from the state snapshot request.
    StateResponse(Box<EngineState>),
}

/// An outbound message from the [crate::InsertTask] to an external actor.
#[derive(Debug, Clone)]
pub enum InsertTaskOut {
    /// Request the sync status.
    SyncStatus,
    /// Request an L2 block info by block number or tag.
    L2BlockInfo(BlockNumberOrTag),
    /// Update the sync status.
    UpdateSyncStatus(SyncStatus),
    /// Request a state snapshot.
    StateSnapshot,
    /// The new payload was invalid.
    InvalidNewPayload,

    // TODO: This needs to be handled by the rollup node as a temporary derivation error.
    // TODO: see: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L389
    /// An error from the `engine_newPayload` call.
    FailedToInsertNewPayload,
    /// Update the engine state with a new [PayloadStatus].
    UpdatePayloadStatus(PayloadStatus),
    /// Failed to process new unsafe payload.
    FailedToProcessNewPayload,

    /// Update engine state with non finalized el sync new payload.
    NewPayloadNotFinalizedUpdate(L2BlockInfo),

    /// Cross safe update.
    CrossSafeUpdate(L2BlockInfo),

    /// Update the engine state with the forkchoice updated state.
    UpdateForkchoiceState(PayloadStatus, L2BlockInfo),

    /// Forkchoice updated event.
    ForkchoiceUpdated,
}
