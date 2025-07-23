//! Contains the error from the gossip builder.

use crate::BehaviourError;
use derive_more::From;
use libp2p::{Multiaddr, PeerId};
use std::net::IpAddr;
use thiserror::Error;

/// An error publishing a payload.
#[derive(Debug, Error)]
pub enum PublishError {
    /// An error occurred publishing the payload.
    #[error("Failed to publish payload: {0}")]
    PublishError(#[from] libp2p::gossipsub::PublishError),
    /// An error occurred when encoding the payload.
    #[error("Failed to encode payload: {0}")]
    EncodeError(#[from] HandlerEncodeError),
}

/// An error occurred when encoding the payload from the block handler.
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
    /// A TCP error.
    #[error("TCP error")]
    TcpError,
    /// An error when setting the behaviour on the swarm builder.
    #[error("error setting behaviour on swarm builder")]
    WithBehaviourError,
    /// An error when building the gossip behaviour.
    #[error("error building gossip behaviour")]
    BehaviourError(BehaviourError),
    /// An error when setting up the sync request/response protocol.
    #[error("error setting up sync request/response protocol")]
    SetupSyncReqRespError,
    /// The sync request/response protocol has already been accepted.
    #[error("sync request/response protocol already accepted")]
    SyncReqRespAlreadyAccepted,
}

/// An error type representing reasons why a peer cannot be dialed.
#[derive(Debug, Clone, Error)]
pub enum DialError {
    /// Failed to extract PeerId from Multiaddr.
    #[error("Failed to extract PeerId from Multiaddr: {addr}")]
    InvalidMultiaddr {
        /// The multiaddress that failed to be parsed or does not contain a valid PeerId component
        addr: Multiaddr,
    },
    /// Already dialing this peer.
    #[error("Already dialing peer: {peer_id}")]
    AlreadyDialing {
        /// The PeerId of the peer that is already being dialed
        peer_id: PeerId,
    },
    /// Dial threshold reached for this peer.
    #[error("Dial threshold reached for peer: {addr}")]
    ThresholdReached {
        /// The multiaddress of the peer that has reached the maximum dial attempts
        addr: Multiaddr,
    },
    /// Peer is blocked.
    #[error("Peer is blocked: {peer_id}")]
    PeerBlocked {
        /// The PeerId of the peer that is on the blocklist
        peer_id: PeerId,
    },
    /// Failed to extract IP address from Multiaddr.
    #[error("Failed to extract IP address from Multiaddr: {addr}")]
    InvalidIpAddress {
        /// The multiaddress that does not contain a valid IP address component
        addr: Multiaddr,
    },
    /// IP address is blocked.
    #[error("IP address is blocked: {ip}")]
    AddressBlocked {
        /// The IP address that is on the blocklist
        ip: IpAddr,
    },
    /// IP address is in a blocked subnet.
    #[error("IP address {ip} is in a blocked subnet")]
    SubnetBlocked {
        /// The IP address that belongs to a blocked subnet range
        ip: IpAddr,
    },
}
