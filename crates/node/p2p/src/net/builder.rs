//! Network Builder Module.

use alloy_primitives::Address;
use alloy_signer_local::PrivateKeySigner;
use discv5::{Config as Discv5Config, Enr};
use kona_genesis::RollupConfig;
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use libp2p::{Multiaddr, identity::Keypair};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{path::PathBuf, time::Duration};
use tokio::sync::broadcast::Sender as BroadcastSender;

use crate::{
    Broadcast, Config, Discv5Builder, GossipDriverBuilder, Network, NetworkBuilderError,
    P2pRpcRequest, discv5::LocalNode, gossip::GaterConfig,
};

/// Constructs a [`Network`] for the OP Stack Consensus Layer.
#[derive(Debug)]
pub struct NetworkBuilder {
    /// The discovery driver.
    discovery: Discv5Builder,
    /// The gossip driver.
    gossip: GossipDriverBuilder,
    /// A receiver for network RPC requests.
    rpc_recv: Option<tokio::sync::mpsc::Receiver<P2pRpcRequest>>,
    /// A broadcast sender for the unsafe block payloads.
    payload_tx: Option<BroadcastSender<OpExecutionPayloadEnvelope>>,
    /// A local signer for payloads.
    local_signer: Option<PrivateKeySigner>,
}

impl From<Config> for NetworkBuilder {
    fn from(config: Config) -> Self {
        Self::new(
            config.rollup_config,
            config.unsafe_block_signer,
            config.gossip_address,
            config.keypair,
            config.discovery_address,
            config.discovery_config,
        )
        .with_discovery_randomize(config.discovery_randomize)
        .with_bootstore(config.bootstore)
        .with_bootnodes(config.bootnodes)
        .with_discovery_interval(config.discovery_interval)
        .with_gossip_config(config.gossip_config)
        .with_peer_scoring(config.scoring)
        .with_peer_monitoring(config.monitor_peers)
        .with_topic_scoring(config.topic_scoring)
        .with_gater_config(config.gater_config)
        .with_local_signer(config.local_signer)
    }
}

impl NetworkBuilder {
    /// Creates a new [`NetworkBuilder`].
    pub const fn new(
        rollup_config: RollupConfig,
        unsafe_block_signer: Address,
        gossip_addr: Multiaddr,
        keypair: Keypair,
        discovery_address: LocalNode,
        discovery_config: discv5::Config,
    ) -> Self {
        Self {
            discovery: Discv5Builder::new(
                discovery_address,
                rollup_config.l2_chain_id,
                discovery_config,
            ),
            gossip: GossipDriverBuilder::new(
                rollup_config,
                unsafe_block_signer,
                gossip_addr,
                keypair,
            ),
            rpc_recv: None,
            payload_tx: None,
            local_signer: None,
        }
    }

    /// Sets the configuration for the connection gater.
    pub fn with_gater_config(self, config: GaterConfig) -> Self {
        Self { gossip: self.gossip.with_gater_config(config), ..self }
    }

    /// Sets the local signer for the [`Network`].
    pub fn with_local_signer(self, local_signer: Option<PrivateKeySigner>) -> Self {
        Self { local_signer, ..self }
    }

    /// Sets the bootstore path for the [`crate::Discv5Driver`].
    pub fn with_bootstore(self, bootstore: Option<PathBuf>) -> Self {
        if let Some(bootstore) = bootstore {
            return Self { discovery: self.discovery.with_bootstore(bootstore), ..self };
        }
        self
    }

    /// Sets the interval at which to randomize discovery peers.
    pub fn with_discovery_randomize(self, randomize: Option<Duration>) -> Self {
        Self { discovery: self.discovery.with_discovery_randomize(randomize), ..self }
    }

    /// Sets the initial bootnodes to add to the bootstore.
    pub fn with_bootnodes(self, bootnodes: Vec<Enr>) -> Self {
        Self { discovery: self.discovery.with_bootnodes(bootnodes), ..self }
    }

    /// Sets the peer scoring based on the given [`PeerScoreLevel`].
    pub fn with_peer_scoring(self, level: PeerScoreLevel) -> Self {
        Self { gossip: self.gossip.with_peer_scoring(level), ..self }
    }

    /// Sets topic scoring for the [`crate::GossipDriver`].
    pub fn with_topic_scoring(self, topic_scoring: bool) -> Self {
        Self { gossip: self.gossip.with_topic_scoring(topic_scoring), ..self }
    }

    /// Sets the peer monitoring for the [`crate::GossipDriver`].
    pub fn with_peer_monitoring(self, peer_monitoring: Option<PeerMonitoring>) -> Self {
        Self { gossip: self.gossip.with_peer_monitoring(peer_monitoring), ..self }
    }

    /// Sets the discovery interval for the [`crate::Discv5Driver`].
    pub fn with_discovery_interval(self, interval: tokio::time::Duration) -> Self {
        Self { discovery: self.discovery.with_interval(interval), ..self }
    }

    /// Sets the address for the [`crate::Discv5Driver`].
    pub fn with_discovery_address(self, address: LocalNode) -> Self {
        Self { discovery: self.discovery.with_local_node(address), ..self }
    }

    /// Sets the gossipsub config for the [`crate::GossipDriver`].
    pub fn with_gossip_config(self, config: libp2p::gossipsub::Config) -> Self {
        Self { gossip: self.gossip.with_config(config), ..self }
    }

    /// Sets the rpc receiver for the [`crate::Network`].
    pub fn with_rpc_receiver(self, rpc_recv: tokio::sync::mpsc::Receiver<P2pRpcRequest>) -> Self {
        Self { rpc_recv: Some(rpc_recv), ..self }
    }

    /// Sets the [`Discv5Config`] for the [`crate::Discv5Driver`].
    pub fn with_discovery_config(self, config: Discv5Config) -> Self {
        Self { discovery: self.discovery.with_discovery_config(config), ..self }
    }

    /// Sets the gossip address for the [`crate::GossipDriver`].
    pub fn with_gossip_address(self, addr: Multiaddr) -> Self {
        Self { gossip: self.gossip.with_address(addr), ..self }
    }

    /// Sets the timeout for the [`crate::GossipDriver`].
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { gossip: self.gossip.with_timeout(timeout), ..self }
    }

    /// Sets the unsafe block sender for the [`crate::Network`].
    pub fn with_unsafe_block_sender(
        self,
        sender: BroadcastSender<OpExecutionPayloadEnvelope>,
    ) -> Self {
        Self { payload_tx: Some(sender), ..self }
    }

    /// Builds the [`Network`].
    pub fn build(mut self) -> Result<Network, NetworkBuilderError> {
        let (gossip, unsafe_block_signer_sender) = self.gossip.build()?;
        let discovery = self.discovery.build()?;
        let rpc = self.rpc_recv.take();
        let payload_tx = self.payload_tx.unwrap_or(tokio::sync::broadcast::channel(256).0);
        let (publish_tx, publish_rx) = tokio::sync::mpsc::channel(256);

        Ok(Network {
            gossip,
            discovery,
            unsafe_block_signer_sender,
            rpc,
            broadcast: Broadcast::new(payload_tx),
            publish_tx,
            publish_rx,
            local_signer: self.local_signer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use discv5::{ConfigBuilder, ListenConfig, enr::CombinedKey};
    use libp2p::gossipsub::IdentTopic;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[derive(Debug)]
    struct NetworkBuilderParams {
        rollup_config: RollupConfig,
        signer: Address,
    }

    impl Default for NetworkBuilderParams {
        fn default() -> Self {
            Self { rollup_config: RollupConfig::default(), signer: Address::random() }
        }
    }

    fn network_builder(params: NetworkBuilderParams) -> NetworkBuilder {
        let keypair = Keypair::generate_secp256k1();
        let signer = params.signer;
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));

        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };

        let discovery_address =
            LocalNode::new(secret_key, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9098, 9098);

        let discovery_config =
            ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9098))
                .build();

        NetworkBuilder::new(
            params.rollup_config,
            signer,
            gossip_addr,
            keypair,
            discovery_address,
            discovery_config,
        )
    }

    #[test]
    fn test_build_simple_succeeds() {
        let signer = Address::random();
        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };
        let disc_listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9097);
        let disc_enr = LocalNode::new(secret_key, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9098, 9098);
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));

        let driver = network_builder(NetworkBuilderParams {
            rollup_config: RollupConfig { l2_chain_id: 10, ..Default::default() },
            signer,
        })
        .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
        .with_gossip_address(gossip_addr.clone())
        .with_discovery_address(disc_enr)
        .with_discovery_config(ConfigBuilder::new(disc_listen.into()).build())
        .build()
        .unwrap();

        // Driver Assertions
        let id = 10;
        assert_eq!(driver.gossip.addr, gossip_addr);
        assert_eq!(driver.discovery.chain_id, id);
        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9098);

        // Block Handler Assertions
        assert_eq!(driver.gossip.handler.rollup_config.l2_chain_id, id);
        let v1 = IdentTopic::new(format!("/optimism/{}/0/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v1_topic.hash(), v1.hash());
        let v2 = IdentTopic::new(format!("/optimism/{}/1/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v2_topic.hash(), v2.hash());
        let v3 = IdentTopic::new(format!("/optimism/{}/2/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v3_topic.hash(), v3.hash());
        let v4 = IdentTopic::new(format!("/optimism/{}/3/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v4_topic.hash(), v4.hash());
    }

    #[test]
    fn test_build_network_custom_configs() {
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));

        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };

        let disc = LocalNode::new(secret_key, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9097, 9097);
        let discovery_config =
            ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9098))
                .build();
        let driver = network_builder(Default::default())
            .with_gossip_address(gossip_addr)
            .with_discovery_address(disc)
            .with_discovery_config(discovery_config)
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .build()
            .unwrap();

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9097);
    }
}
