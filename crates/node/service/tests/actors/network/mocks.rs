//! Tests interactions with sequencer actor's inputs channels.

use alloy_chains::Chain;
use alloy_signer::k256;

use alloy_primitives::Address;
use discv5::{ConfigBuilder, Enr, ListenConfig};
use kona_disc::LocalNode;
use kona_genesis::RollupConfig;
use kona_gossip::{P2pRpcRequest, PeerDump, PeerInfo};
use kona_node_service::{
    NetworkActor, NetworkActorError, NetworkBuilder, NetworkContext, NetworkInboundData, NodeActor,
};
use libp2p::{Multiaddr, identity::Keypair, multiaddr::Protocol};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::str::FromStr;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::actors::utils::SeedGenerator;

pub(super) struct TestNetwork {
    inbound_data: NetworkInboundData,
    /// We'll remove those fields as we add more tests.
    #[allow(dead_code)]
    blocks_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
    #[allow(dead_code)]
    handle: JoinHandle<Result<(), NetworkActorError>>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TestNetworkError {
    #[error("P2p receiver closed")]
    P2pReceiverClosed,
    #[error("P2p receiver closed before sending response: {0}")]
    OneshotError(#[from] oneshot::error::RecvError),
    #[error("Peer info missing ENR")]
    PeerInfoMissingEnr,
    #[error("Invalid ENR: {0}")]
    InvalidEnr(String),
    #[error("Peer not connected")]
    PeerNotConnected,
}

fn rollup_config() -> RollupConfig {
    RollupConfig { l2_chain_id: Chain::from_id(19_934_000), ..Default::default() }
}

impl TestNetwork {
    pub(super) fn new(bootnodes: Vec<Enr>, seed_generator: &mut SeedGenerator) -> Self {
        let unsafe_block_signer = Address::ZERO;
        let keypair = Keypair::generate_secp256k1();
        let secp256k1_key = keypair.clone().try_into_secp256k1()
        .map_err(|e| anyhow::anyhow!("Impossible to convert keypair to secp256k1. This is a bug since we only support secp256k1 keys: {e}")).unwrap()
        .secret().to_bytes();
        let local_node_key = k256::ecdsa::SigningKey::from_bytes(&secp256k1_key.into())
        .map_err(|e| anyhow::anyhow!("Impossible to convert keypair to k256 signing key. This is a bug since we only support secp256k1 keys: {e}")).unwrap();

        let node_addr = seed_generator.next_loopback_address();

        let discovery_port = seed_generator.next_port();

        let discovery_config = ConfigBuilder::new(ListenConfig::from_ip(node_addr, discovery_port))
            // Only allow loopback addresses.
            .table_filter(|enr| {
                let Some(ip) = enr.ip4() else {
                    return false;
                };

                ip.is_loopback()
            })
            .build();

        let gossip_port = seed_generator.next_port();
        let mut gossip_multiaddr = Multiaddr::from(node_addr);
        gossip_multiaddr.push(Protocol::Tcp(gossip_port));

        let gossip_config = kona_gossip::default_config_builder().build().unwrap();

        // Create a new network actor. No external connections
        let builder = NetworkBuilder::new(
            // Create a new rollup config. We don't need to specify any of the fields.
            rollup_config(),
            unsafe_block_signer,
            gossip_multiaddr,
            keypair,
            LocalNode::new(local_node_key, node_addr, gossip_port, discovery_port),
            discovery_config,
        )
        .with_bootnodes(bootnodes)
        .with_gossip_config(gossip_config);

        let (inbound_data, actor) = NetworkActor::new(builder);

        let (blocks_tx, blocks_rx) = mpsc::channel(1024);
        let cancellation = CancellationToken::new();

        let context = NetworkContext { blocks: blocks_tx, cancellation };

        let handle = tokio::spawn(async move { actor.start(context).await });

        Self { inbound_data, blocks_rx, handle }
    }

    pub(super) async fn peer_info(&self) -> Result<PeerInfo, TestNetworkError> {
        // Try to get the peer info. Send a peer info request to the network actor.
        let (peer_info_tx, peer_info_rx) = oneshot::channel();
        let peer_info_request = P2pRpcRequest::PeerInfo(peer_info_tx);
        self.inbound_data
            .p2p_rpc
            .send(peer_info_request)
            .await
            .map_err(|_| TestNetworkError::P2pReceiverClosed)?;

        let info = peer_info_rx.await?;

        Ok(info)
    }

    pub(super) async fn peers(&self) -> Result<PeerDump, TestNetworkError> {
        let (peers_tx, peers_rx) = oneshot::channel();
        let peers_request = P2pRpcRequest::Peers { out: peers_tx, connected: true };
        self.inbound_data
            .p2p_rpc
            .send(peers_request)
            .await
            .map_err(|_| TestNetworkError::P2pReceiverClosed)?;
        let peers = peers_rx.await?;
        Ok(peers)
    }

    pub(super) async fn is_connected_to(&self, other: &Self) -> Result<(), TestNetworkError> {
        let other_peer_id = other.peer_id().await?;
        let peers = self.peers().await?;
        if !peers.peers.contains_key(&other_peer_id) {
            return Err(TestNetworkError::PeerNotConnected);
        }
        Ok(())
    }

    pub(super) async fn peer_enr(&self) -> Result<Enr, TestNetworkError> {
        let enr = self.peer_info().await?.enr.ok_or(TestNetworkError::PeerInfoMissingEnr)?;
        // Parse the ENR
        let enr = Enr::from_str(&enr).map_err(TestNetworkError::InvalidEnr)?;
        Ok(enr)
    }

    pub(super) async fn peer_id(&self) -> Result<String, TestNetworkError> {
        Ok(self.peer_info().await?.peer_id)
    }
}
