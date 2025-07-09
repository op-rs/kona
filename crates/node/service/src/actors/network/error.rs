//! Contains the error type for the network driver builder.

use kona_p2p::{Discv5BuilderError, GossipDriverBuilderError};

/// An error from the [`crate::NetworkBuilder`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkBuilderError {
    /// An error from building the gossip driver.
    #[error(transparent)]
    GossipDriverBuilder(#[from] GossipDriverBuilderError),
    /// An error from building the discv5 driver.
    #[error(transparent)]
    DiscoveryDriverBuilder(#[from] Discv5BuilderError),
}
