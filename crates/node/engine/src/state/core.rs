//! The internal state of the engine controller.

use crate::SyncStatus;
use alloy_rpc_types_engine::ForkchoiceState;
use kona_protocol::L2BlockInfo;

/// The chain state viewed by the engine controller.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineState {
    /// Most recent block found on the p2p network
    pub(crate) unsafe_head: L2BlockInfo,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    pub(crate) cross_unsafe_head: L2BlockInfo,
    /// Pending localSafeHead
    /// L2 block processed from the middle of a span batch,
    /// but not marked as the safe block yet.
    pub(crate) pending_safe_head: L2BlockInfo,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    pub(crate) local_safe_head: L2BlockInfo,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    pub(crate) safe_head: L2BlockInfo,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    pub(crate) finalized_head: L2BlockInfo,
    /// The unsafe head to roll back to,
    /// after the pending safe head fails to become safe.
    /// This is changing in the Holocene fork.
    pub(crate) backup_unsafe_head: Option<L2BlockInfo>,

    /// The [SyncStatus] of the engine.
    pub sync_status: SyncStatus,

    /// If a forkchoice update call is needed.
    pub forkchoice_update_needed: bool,

    /// Track when the rollup node changes the forkchoice to restore previous
    /// known unsafe chain. e.g. Unsafe Reorg caused by Invalid span batch.
    /// This update does not retry except engine returns non-input error
    /// because engine may forgot backupUnsafeHead or backupUnsafeHead is not part
    /// of the chain.
    pub need_fcu_call_backup_unsafe_reorg: bool,
}

impl EngineState {
    /// Creates a `ForkchoiceState`
    ///
    /// - `head_block` = `unsafe_head`
    /// - `safe_block` = `safe_head`
    /// - `finalized_block` = `finalized_head`
    ///
    /// If the block info is not yet available, the default values are used.
    pub const fn create_forkchoice_state(&self) -> ForkchoiceState {
        ForkchoiceState {
            head_block_hash: self.unsafe_head.block_info.hash,
            safe_block_hash: self.safe_head.block_info.hash,
            finalized_block_hash: self.finalized_head.block_info.hash,
        }
    }

    /// Returns the current unsafe head.
    pub const fn unsafe_head(&self) -> L2BlockInfo {
        self.unsafe_head
    }

    /// Returns the current cross-verified unsafe head.
    pub const fn cross_unsafe_head(&self) -> L2BlockInfo {
        self.cross_unsafe_head
    }

    /// Returns the current pending safe head.
    pub const fn pending_safe_head(&self) -> L2BlockInfo {
        self.pending_safe_head
    }

    /// Returns the current local safe head.
    pub const fn local_safe_head(&self) -> L2BlockInfo {
        self.local_safe_head
    }

    /// Returns the current safe head.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.safe_head
    }

    /// Returns the current finalized head.
    pub const fn finalized_head(&self) -> L2BlockInfo {
        self.finalized_head
    }

    /// Returns the current backup unsafe head.
    pub const fn backup_unsafe_head(&self) -> Option<L2BlockInfo> {
        self.backup_unsafe_head
    }

    /// Set the unsafe head.
    pub fn set_unsafe_head(&mut self, unsafe_head: L2BlockInfo) {
        self.unsafe_head = unsafe_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the cross-verified unsafe head.
    pub fn set_cross_unsafe_head(&mut self, cross_unsafe_head: L2BlockInfo) {
        self.cross_unsafe_head = cross_unsafe_head;
    }

    /// Set the pending safe head.
    pub fn set_pending_safe_head(&mut self, pending_safe_head: L2BlockInfo) {
        self.pending_safe_head = pending_safe_head;
    }

    /// Set the local safe head.
    pub fn set_local_safe_head(&mut self, local_safe_head: L2BlockInfo) {
        self.local_safe_head = local_safe_head;
    }

    /// Set the safe head.
    pub fn set_safe_head(&mut self, safe_head: L2BlockInfo) {
        self.safe_head = safe_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the finalized head.
    pub fn set_finalized_head(&mut self, finalized_head: L2BlockInfo) {
        self.finalized_head = finalized_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the backup unsafe head.
    pub fn set_backup_unsafe_head(&mut self, backup_unsafe_head: L2BlockInfo, reorg: bool) {
        self.backup_unsafe_head = Some(backup_unsafe_head);
        self.need_fcu_call_backup_unsafe_reorg = reorg;
    }
}
