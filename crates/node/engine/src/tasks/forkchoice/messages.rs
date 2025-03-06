//! Contains the message types for the forkchoice task.

use alloy_rpc_types_engine::ForkchoiceUpdated;

use crate::{EngineState, SyncStatus};

/// An inbound message from an external actor to the [crate::ForkchoiceTask].
#[derive(Debug, Clone)]
pub enum ForkchoiceTaskInput {
    /// A response from the state request.
    ///
    /// This contains a snapshot of the state.
    StateResponse(Box<EngineState>),
    /// A response from the sync status request.
    SyncStatusResponse(SyncStatus),
}

/// An outbound message from the [crate::ForkchoiceTask] to an external actor.
#[derive(Debug, Clone)]
pub enum ForkchoiceTaskOut {
    /// A request for a snapshot of the state.
    StateSnapshot,
    /// Request the sync status.
    SyncStatus,
    /// Instruct the state to update the backup unsafe head.
    UpdateBackupUnsafeHead,
    /// Instruct the state that a forkchoice update is not needed.
    ForkchoiceNotNeeded,
    /// A message confirming that the forkchoice update was successful.
    ForkchoiceUpdated(ForkchoiceUpdated),
}
