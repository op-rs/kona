//! Contains the `SyncStatus` enum that tracks the node's sync status.

/// Sync Status
///
/// Syncing the consensus node can be done in two ways:
/// 1. Consensus Layer Sync: Syncing through the consensus layer and verifying through the engine
///    api.
/// 2. Execution Layer Sync: Syncing through the execution engine.
///
/// Syncing through the execution layer is linearly broken down into 4 states:
/// 1. The sync will start.
/// 2. The sync has started.
/// 3. The sync has finished but the last block is not finalized.
/// 4. The sync has finished.
///
/// Note, execution layer sync is only done if there is no finalized block.
/// Once the execution layer sync has finished, the last block must be marked
/// as finalized and consolidation is performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SyncStatus {
    /// Consensus sync
    ConsensusLayer = 0,
    /// Execution sync will start.
    /// This is to verify that nothing has been finalized yet.
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
    pub fn has_started(&self) -> bool {
        matches!(self, SyncStatus::ExecutionLayerStarted)
    }

    /// Returns if syncing is in progress.
    pub fn is_syncing(&self) -> bool {
        matches!(
            self,
            SyncStatus::ExecutionLayerWillStart |
                SyncStatus::ExecutionLayerStarted |
                SyncStatus::ExecutionLayerNotFinalized
        )
    }
}

impl From<crate::sync::SyncMode> for SyncStatus {
    fn from(mode: crate::sync::SyncMode) -> Self {
        match mode {
            crate::sync::SyncMode::ConsensusLayer => SyncStatus::ConsensusLayer,
            crate::sync::SyncMode::ExecutionLayer => SyncStatus::ExecutionLayerWillStart,
        }
    }
}
