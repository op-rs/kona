//! Contains the error from the gossip builder.

use crate::BehaviourError;
use derive_more::From;
use thiserror::Error;

/// An error publishing a payload.
#[derive(Debug, Error)]
pub enum PublishError {
    /// An error occurred publishing the payload.
    #[error("Failed to publish payload: {0}")]
    PublishError(#[from] libp2p::gossipsub::PublishError),
    /// An error occured when encoding the payload.
    #[error("Failed to encode payload: {0}")]
    EncodeError(#[from] HandlerEncodeError),
}

/// An error occured when encoding the payload from the block handler.
#[derive(Debug, Error)]
pub enum HandlerEncodeError {
    /// Failed to encode the payload envelope.
    #[error("Failed to encode payload: {0}")]
    PayloadEncodeError(#[from] op_alloy_rpc_types_engine::PayloadEnvelopeEncodeError),
    /// The topic is unknown.
    #[error("Unknown topic: {0}")]
    UnknownTopic(libp2p::gossipsub::TopicHash),
}

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
}
