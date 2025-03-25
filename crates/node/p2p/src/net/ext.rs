//! Contains an externally-facing api for the [`crate::Network`].
//!
//! The external api is used to service requests made by the RPC.

use kona_rpc::PeerInfo;
use tokio::sync::{mpsc::Receiver, oneshot::Sender};

/// A network RPC Request.
#[derive(Debug)]
pub enum NetRpcRequest {
    /// Returns [`PeerInfo`] for the [`crate::Network`].
    PeerInfo(Sender<PeerInfo>),
}

/// An RPC request handler for the [`crate::Network`].
#[derive(Debug)]
pub struct NetworkRpcHandler {
    /// The inner [`Receiver`]
    pub receiver: Receiver<NetRpcRequest>,
}

impl Default for NetworkRpcHandler {
    fn default() -> Self {
        let (_, receiver) = tokio::sync::mpsc::channel(1);
        Self { receiver }
    }
}

impl NetworkRpcHandler {
    /// Constructs a new [`NetworkRpcHandler`] given a receiver channel.
    pub const fn new(receiver: Receiver<NetRpcRequest>) -> Self {
        Self { receiver }
    }
}

impl NetworkRpcHandler {
    /// Receives on the inner channel and handles the request.
    pub async fn recv(&mut self) -> Option<NetRpcRequest> {
        self.receiver.recv().await
    }

    /// Handles a [`NetRpcRequest`].
    pub fn handle_rpc_req(&self, req: NetRpcRequest) {
        match req {
            NetRpcRequest::PeerInfo(sender) => {
                if let Err(e) = sender.send(self.peer_info()) {
                    warn!("Failed to send peer info through response channel: {:?}", e);
                }
            }
        }
    }

    /// Returns the [`PeerInfo`].
    pub fn peer_info(&self) -> PeerInfo {
        todo!()
    }
}
