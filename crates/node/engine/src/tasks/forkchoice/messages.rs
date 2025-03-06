//! Contains the message types for the forkchoice task.

use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated};
use op_alloy_rpc_types_engine::OpPayloadAttributes;

use crate::{EngineForkchoiceVersion, EngineState, SyncStatus};

/// An inbound message from an external actor to the [crate::ForkchoiceTask].
#[derive(Debug, Clone)]
pub enum ForkchoiceTaskInput {
    /// A response from the state request.
    ///
    /// This contains a snapshot of the state.
    StateResponse(Box<EngineState>),
    /// A response from the sync status request.
    SyncStatusResponse(SyncStatus),
    /// Tells the [crate::ForkchoiceTask] that the engine api forkchoice update was valid.
    ///
    /// Provides the returned [ForkchoiceUpdated] response from the `engine_forkchoiceUpdated` call.
    ForkchoiceUpdated(ForkchoiceUpdated),
    /// Tells the [crate::ForkchoiceTask] that the forkchoice update call failed.
    ForkchoiceUpdateFailed,
}

/// An outbound message from the [crate::ForkchoiceTask] to an external actor.
#[derive(Debug, Clone)]
pub enum ForkchoiceTaskOut {
    /// A request for a snapshot of the state.
    StateSnapshot,
    /// Request the sync status.
    SyncStatus,
    /// A message to update the forkchoice.
    ExecuteForkchoiceUpdate(EngineForkchoiceVersion, ForkchoiceState, Option<OpPayloadAttributes>),
    /// Instruct the state to update the backup unsafe head.
    UpdateBackupUnsafeHead,
    /// Instruct the state that a forkchoice update is not needed.
    ForkchoiceNotNeeded,
    /// A message confirming that the forkchoice update was successful.
    ForkchoiceUpdated(ForkchoiceUpdated),
}
