//! The sync config.

use crate::sync::SyncMode;

/// The sync config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConfig {
    /// The mode to sync.
    pub sync_mode: SyncMode,
    /// Skips sanity checks across the L1 origins of unsafe L2 blocks.
    /// This is done when determining the sync-starting point.
    ///
    /// L1 origin verification is deferred.
    /// It's recommended to use this when specifying execution layer sync mode
    /// on the consensus node, and snap sync mode on the execution client.
    ///
    /// Warning: This will be deprecated when checkpoints are implemented.
    /// Note: We probably need to detect the condition that snap sync has not completed
    /// when we do a restart prior to running sync-start if we are doing
    /// snap sync with a genesis finalization data.
    pub skip_sync_start_check: bool,
    /// Supports post finalization EL sync.
    pub supports_post_finalization_elsync: bool,
}
