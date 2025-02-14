//! Common sync types

use alloy_eips::BlockNumHash;
use kona_protocol::BlockInfo;

/// The block reference for an L2 block.
///
/// See: <https://github.com/ethereum-optimism/optimism/blob/develop/op-service/eth/id.go#L33>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct L2BlockRef {
    /// The l1 block info.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub l1_block_info: BlockInfo,
    /// The origin on L1.
    #[cfg_attr(feature = "serde", serde(rename = "l1origin"))]
    pub l1_origin: BlockNumHash,
    /// The sequence number.
    pub sequence_number: u64,
}

/// The [`SyncStatus`][ss] of an Optimism Rollup Node.
///
/// [ss]: https://github.com/ethereum-optimism/optimism/blob/develop/op-service/eth/sync_status.go#L5
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncStatus {
    /// The current L1 block.
    pub current_l1: BlockInfo,
    /// The current L1 finalized block.
    pub current_l1_finalized: BlockInfo,
    /// The L1 head block ref.
    pub head_l1: BlockInfo,
    /// The L1 safe head block ref.
    pub safe_l1: BlockInfo,
    /// The finalized L1 block ref.
    pub finalized_l1: BlockInfo,
    /// The unsafe L2 block ref.
    pub unsafe_l2: L2BlockRef,
    /// The safe L2 block ref.
    pub safe_l2: L2BlockRef,
    /// The finalized L2 block ref.
    pub finalized_l2: L2BlockRef,
    /// The pending safe L2 block ref.
    pub pending_safe_l2: L2BlockRef,
}
