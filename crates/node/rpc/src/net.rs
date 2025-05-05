//! Network types

use kona_p2p::P2pRpcRequest;

use crate::rollup::RollupRpcRequest;

/// A type alias for the sender of a [`P2pRpcRequest`].
type P2pReqSender = tokio::sync::mpsc::Sender<P2pRpcRequest>;
type RollupReqSender = tokio::sync::mpsc::Sender<RollupRpcRequest>;

/// NetworkRpc
///
/// This is a server implementation of [`crate::OpP2PApiServer`].
#[derive(Debug)]
pub struct NetworkRpc {
    /// The channel to send [`P2pRpcRequest`]s.
    pub p2p_sender: P2pReqSender,
    /// The channel to send [`RollupRpcRequest`]s.
    pub rollup_sender: RollupReqSender,
}

impl NetworkRpc {
    /// Constructs a new [`NetworkRpc`] given a sender channel.
    pub const fn new(p2p_sender: P2pReqSender, rollup_sender: RollupReqSender) -> Self {
        Self { p2p_sender, rollup_sender }
    }
}
