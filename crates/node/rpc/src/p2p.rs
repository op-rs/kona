//! RPC Module to serve the P2P API.
//!
//! Kona's P2P RPC API is a JSON-RPC API compatible with the [op-node] API.
//!
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/7a6788836984996747193b91901a824c39032bd8/op-node/p2p/rpc_api.go#L45

use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_p2p::{P2pRpcRequest, PeerCount, PeerDump, PeerInfo, PeerStats};
use std::{net::IpAddr, str::FromStr};

use crate::{OpP2PApiServer, net::NetworkRpc};

#[async_trait]
impl OpP2PApiServer for NetworkRpc {
    async fn opp2p_self(&self) -> RpcResult<PeerInfo> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_self");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::PeerInfo(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_peer_count(&self) -> RpcResult<PeerCount> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_peerCount");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::PeerCount(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        let (connected_discovery, connected_gossip) =
            rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        Ok(PeerCount { connected_discovery, connected_gossip })
    }

    async fn opp2p_peers(&self, connected: bool) -> RpcResult<PeerDump> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_peers");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::Peers { out: tx, connected })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        let dump = rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        Ok(dump)
    }

    async fn opp2p_peer_stats(&self) -> RpcResult<PeerStats> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::PeerStats(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        let stats = rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        Ok(stats)
    }

    async fn opp2p_discovery_table(&self) -> RpcResult<Vec<String>> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_discoveryTable");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::DiscoveryTable(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_block_peer(&self, peer_id: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_blockPeer");
        let id = libp2p::PeerId::from_str(&peer_id)
            .map_err(|_| ErrorObject::from(ErrorCode::InvalidParams))?;
        self.sender
            .send(P2pRpcRequest::BlockPeer { id })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_unblock_peer(&self, peer_id: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_unblockPeer");
        let id = libp2p::PeerId::from_str(&peer_id)
            .map_err(|_| ErrorObject::from(ErrorCode::InvalidParams))?;
        self.sender
            .send(P2pRpcRequest::UnblockPeer { id })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_list_blocked_peers(&self) -> RpcResult<Vec<String>> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_listBlockedPeers");
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.sender
            .send(P2pRpcRequest::ListBlockedPeers(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await
            .map(|peers| peers.iter().map(|p| p.to_string()).collect())
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_block_addr(&self, _ip: IpAddr) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_blockAddr");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unblock_addr(&self, _ip: IpAddr) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_unblockAddr");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_list_blocked_addrs(&self) -> RpcResult<Vec<IpAddr>> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_listBlockedAddrs");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_block_subnet(&self, _subnet: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_blockSubnet");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unblock_subnet(&self, _subnet: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_unblockSubnet");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_list_blocked_subnets(&self) -> RpcResult<Vec<String>> {
        kona_macros::inc!(
            gauge,
            kona_p2p::Metrics::RPC_CALLS,
            "method" => "opp2p_listBlockedSubnets"
        );
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_protect_peer(&self, _peer: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_protectPeer");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_unprotect_peer(&self, _peer: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_unprotectPeer");
        // Method not supported yet.
        Err(ErrorObject::from(ErrorCode::MethodNotFound))
    }

    async fn opp2p_connect_peer(&self, _peer: String) -> RpcResult<()> {
        use std::str::FromStr;
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_connectPeer");
        let ma = libp2p::Multiaddr::from_str(&_peer)
            .map_err(|_| ErrorObject::from(ErrorCode::InvalidParams))?;
        self.sender
            .send(P2pRpcRequest::ConnectPeer { address: ma })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn opp2p_disconnect_peer(&self, peer_id: String) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "opp2p_disconnectPeer");
        let peer_id = match peer_id.parse() {
            Ok(id) => id,
            Err(err) => {
                warn!(target: "rpc", ?err, ?peer_id, "Failed to parse peer ID");
                return Err(ErrorObject::from(ErrorCode::InvalidParams))
            }
        };
        self.sender
            .send(P2pRpcRequest::DisconnectPeer { peer_id })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_multiaddr_string() {
        use std::str::FromStr;
        let ma = "/ip4/127.0.0.1/udt";
        let multiaddr = libp2p::Multiaddr::from_str(ma).unwrap();
        let components = multiaddr.iter().collect::<Vec<_>>();
        assert_eq!(
            components[0],
            libp2p::multiaddr::Protocol::Ip4(std::net::Ipv4Addr::new(127, 0, 0, 1))
        );
        assert_eq!(components[1], libp2p::multiaddr::Protocol::Udt);
    }
}
