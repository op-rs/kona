//! Network types

use kona_p2p::P2pRpcRequest;

/// A type alias for the sender of a [`P2pRpcRequest`].
type P2pReqSender = tokio::sync::mpsc::Sender<P2pRpcRequest>;

/// NetworkRpc
///
/// This is a server implementation of [`crate::OpP2PApiServer`].
#[derive(Debug)]
pub struct NetworkRpc {
    /// The channel to send [`P2pRpcRequest`]s.
    pub sender: P2pReqSender,
}

impl NetworkRpc {
    /// Constructs a new [`NetworkRpc`] given a sender channel.
    pub const fn new(sender: P2pReqSender) -> Self {
        Self { sender }
    }
}
