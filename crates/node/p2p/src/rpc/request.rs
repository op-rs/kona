//! Contains the network RPC request type.

use kona_rpc::PeerInfo;
use tokio::sync::oneshot::Sender;

/// A network RPC Request.
#[derive(Debug)]
pub enum NetRpcRequest {
    /// Returns [`PeerInfo`] for the [`crate::Network`].
    PeerInfo(Sender<PeerInfo>),
}
