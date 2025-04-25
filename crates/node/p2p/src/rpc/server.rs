//! RPC Module to serve the P2P API.
//!
//! Kona's P2P RPC API is a JSON-RPC API compatible with the [op-node] API.
//!
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/7a6788836984996747193b91901a824c39032bd8/op-node/p2p/rpc_api.go#L45

use crate::NetRpcRequest;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_rpc::{OpP2PApiServer, PeerCount, PeerDump, PeerInfo, PeerStats};
use std::net::IpAddr;

/// A type alias for the sender of a [`NetRpcRequest`].
type RpcReqSender = tokio::sync::mpsc::Sender<NetRpcRequest>;

/// NetworkRpc
///
/// This is a server implementation of [`OpP2PApiServer`].
#[derive(Debug)]
pub struct NetworkRpc {
    /// The channel to send [`NetRpcRequest`]s.
    pub sender: RpcReqSender,
}

impl NetworkRpc {
    /// Constructs a new [`NetworkRpc`] given a sender channel.
    pub const fn new(sender: RpcReqSender) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl OpP2PApiServer for NetworkRpc {
    async fn opp2p_self(&self) -> RpcResult<PeerInfo> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(NetRpcRequest::PeerInfo(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_peer_count(&self) -> RpcResult<PeerCount> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(NetRpcRequest::PeerCount(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        let (connected_discovery, connected_gossip) =
            rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        Ok(PeerCount { connected_discovery, connected_gossip })
    }

    async fn opp2p_peers(&self) -> RpcResult<PeerDump> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_peer_stats(&self) -> RpcResult<PeerStats> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_discovery_table(&self) -> RpcResult<Vec<String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(NetRpcRequest::DiscoveryTable(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_block_peer(&self, _peer: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_list_blocked_peers(&self) -> RpcResult<Vec<String>> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_block_addr(&self, _ip: IpAddr) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unblock_addr(&self, _ip: IpAddr) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_list_blocked_addrs(&self) -> RpcResult<Vec<IpAddr>> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_block_subnet(&self, _subnet: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unblock_subnet(&self, _subnet: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_list_blocked_subnets(&self) -> RpcResult<Vec<String>> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_protect_peer(&self, _peer: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unprotect_peer(&self, _peer: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_connect_peer(&self, _peer: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_disconnect_peer(&self, _peer: String) -> RpcResult<()> {
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }
}
