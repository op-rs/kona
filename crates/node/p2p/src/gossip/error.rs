//! Contains the error from the gossip builder.

use crate::BehaviourError;
use derive_more::From;
use thiserror::Error;

/// An error type for the [`crate::GossipDriverBuilder`].
#[derive(Debug, Clone, PartialEq, Eq, From, Error)]
pub enum GossipDriverBuilderError {
    /// Missing chain id.
    #[error("missing chain id")]
    MissingChainID,
    /// The unsafe block signer is missing.
    #[error("missing unsafe block signer")]
    MissingUnsafeBlockSigner,
    /// A TCP error.
    #[error("TCP error")]
    TcpError,
    /// An error when setting the behaviour on the swarm builder.
    #[error("error setting behaviour on swarm builder")]
    WithBehaviourError,
    /// Missing gossip address.
    #[error("gossip address not set")]
    GossipAddrNotSet,
    /// An error when building the gossip behaviour.
    #[error("error building gossip behaviour")]
    BehaviourError(BehaviourError),
    /// Missing the l2 block time.
    #[error("missing l2 block time")]
    MissingL2BlockTime,
}
