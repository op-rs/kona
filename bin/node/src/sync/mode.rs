//! Contains the `SyncMode`.

use std::fmt::Display;

/// Sync Mode
///
/// Syncing the consensus node can be done in two ways:
/// 1. Consensus Layer Sync: the node fully drives the execution client and imports unsafe blocks &
///    fetches unsafe blocks that it has missed.
/// 2. Execution Layer Sync: the node tells the execution client to sync towards the tip of the
///    chain. It will consolidate the chain as usual. This allows execution clients to snap sync if
///    they are capable of it.
///
/// This is ported from: <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/sync/config.go#L15>.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SyncMode {
    /// Syncing the consensus layer
    ConsensusLayer = 0,
    /// Syncing the execution layer
    ExecutionLayer = 1,
}

impl AsRef<str> for SyncMode {
    fn as_ref(&self) -> &str {
        match self {
            SyncMode::ConsensusLayer => "consensus-layer",
            SyncMode::ExecutionLayer => "execution-layer",
        }
    }
}

impl Display for SyncMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
