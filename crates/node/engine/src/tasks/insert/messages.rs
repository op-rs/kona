//! Contains the message types for the insert task.

use crate::SyncStatus;
use alloy_eips::eip1898::BlockNumberOrTag;
use kona_protocol::L2BlockInfo;

/// An inbound message from an external actor to the [crate::InsertTask].
#[derive(Debug, Clone)]
pub enum InsertTaskInput {
    /// A response from the sync status request.
    SyncStatusResponse(SyncStatus),
    /// A response from feching L2 block info.
    L2BlockInfoResponse(L2BlockInfo),
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
}
