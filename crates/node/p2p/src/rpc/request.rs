//! Contains the network RPC request type.

use kona_rpc::PeerInfo;
use tokio::sync::oneshot::Sender;

/// A network RPC Request.
#[derive(Debug)]
pub enum NetRpcRequest {
    /// Returns [`PeerInfo`] for the [`crate::Network`].
    PeerInfo(Sender<PeerInfo>),
    /// Returns the current peer count for both the
    /// - Discovery Service ([`crate::Discv5Driver`])
    /// - Gossip Service ([`crate::GossipDriver`])
    PeerCount(Sender<(Option<usize>, usize)>),
}
