//! Contains the different kinds of execution engine clients that can be used.

use derive_more::{Display, FromStr};

/// The engine kind identifies the engine client's kind,
/// used to control the behavior of optimism in different engine clients.
#[derive(Debug, Display, FromStr, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Geth engine client.
    #[display("geth")]
    Geth,
    /// Reth engine client.
    #[display("reth")]
    Reth,
    /// Erigon engine client.
    #[display("erigon")]
    Erigon,
}

impl EngineKind {
    /// Contains all valid engine client kinds.
    pub const KINDS: [Self; 3] = [Self::Geth, Self::Reth, Self::Erigon];

    /// Returns whether the engine client kind supports post finalization EL sync.
    pub const fn supports_post_finalization_elsync(self) -> bool {
        match self {
            Self::Geth => false,
            Self::Erigon | Self::Reth => true,
        }
    }
}
