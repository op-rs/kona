//! Contains the [`SyncStatus`] enum that tracks the node's sync status.

/// Sync Status
///
/// Syncing an L2 consensus node is possible in two ways:
///
/// 1. Consensus Layer Sync: Syncing through the consensus layer and verifying through the engine
///    api. (UNSUPPORTED).
/// 2. Execution Layer Sync: Syncing through the execution engine.
///
/// Kona only supports execution layer sync.
///
/// Syncing through the execution layer is linearly broken down into 4 states:
///
/// 1. Sync hasn't started yet. [`SyncStatus::ExecutionLayerWillStart`]
/// 2. The sync has started. [`SyncStatus::ExecutionLayerStarted`]
/// 3. The sync has finished but the last block is not finalized.
///    [`SyncStatus::ExecutionLayerNotFinalized`]
/// 4. The sync has finished. [`SyncStatus::ExecutionLayerFinished`]
///
/// Once the execution layer sync has finished, the last block is marked
/// as finalized and consolidation is performed.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SyncStatus {
    /// Execution sync will start.
    /// This is to verify that nothing has been finalized yet.
    #[default]
    ExecutionLayerWillStart = 1,
    /// Execution sync has started.
    ExecutionLayerStarted = 2,
    /// Execution sync has finished but the last block is not finalized.
    ExecutionLayerNotFinalized = 3,
    /// Execution sync has finished.
    /// At this point, consolidation is being performed.
    ExecutionLayerFinished = 4,
}

impl SyncStatus {
    /// Returns if the execution layer sync has started.
    pub const fn has_started(&self) -> bool {
        matches!(self, Self::ExecutionLayerStarted)
    }

    /// Returns if syncing is in progress.
    pub const fn is_syncing(&self) -> bool {
        matches!(
            self,
            Self::ExecutionLayerWillStart |
                Self::ExecutionLayerStarted |
                Self::ExecutionLayerNotFinalized
        )
    }
}
