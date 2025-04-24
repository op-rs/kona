//! Network Builder Module.

use alloy_primitives::Address;
use discv5::{Config as Discv5Config, Enr};
use kona_genesis::RollupConfig;
use libp2p::{Multiaddr, identity::Keypair};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::{path::PathBuf, time::Duration};
use tokio::sync::broadcast::Sender as BroadcastSender;

use crate::{
    Broadcast, Config, Discv5Builder, GossipDriverBuilder, NetRpcRequest, Network,
    NetworkBuilderError, PeerMonitoring, PeerScoreLevel, discv5::AdvertisedIpAndPort,
};

/// Constructs a [`Network`] for the OP Stack Consensus Layer.
#[derive(Debug, Default)]
pub struct NetworkBuilder {
    /// The discovery driver.
    discovery: Discv5Builder,
    /// The gossip driver.
    gossip: GossipDriverBuilder,
    /// The unsafe block signer [`Address`].
    signer: Option<Address>,
    /// The [`RollupConfig`] only used to select which topic to publish blocks to.
    cfg: Option<RollupConfig>,
    /// A receiver for network RPC requests.
    rpc_recv: Option<tokio::sync::mpsc::Receiver<NetRpcRequest>>,
    /// A broadcast sender for the unsafe block payloads.
    payload_tx: Option<BroadcastSender<OpNetworkPayloadEnvelope>>,
    /// A receiver for unsafe blocks to publish.
    publish_rx: Option<tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>>,
}

impl From<Config> for NetworkBuilder {
    fn from(config: Config) -> Self {
        Self::new()
            .with_discovery_config(config.discovery_config)
            .with_bootstore(config.bootstore)
            .with_bootnodes(config.bootnodes)
            .with_discovery_interval(config.discovery_interval)
            .with_discovery_address(config.discovery_address)
            .with_gossip_address(config.gossip_address)
            .with_unsafe_block_signer(config.unsafe_block_signer)
            .with_gossip_config(config.gossip_config)
            .with_peer_scoring(config.scoring)
            .with_block_time(config.block_time)
            .with_keypair(config.keypair)
            .with_peer_redial(config.redial)
    }
}

impl NetworkBuilder {
    /// Creates a new [`NetworkBuilder`].
    pub const fn new() -> Self {
        Self {
            discovery: Discv5Builder::new(),
            gossip: GossipDriverBuilder::new(),
            signer: None,
            rpc_recv: None,
            payload_tx: None,
            publish_rx: None,
            cfg: None,
        }
    }

    /// Sets the number of times to redial a peer.
    pub fn with_peer_redial(self, redial: Option<u64>) -> Self {
        Self { gossip: self.gossip.with_peer_redial(redial), ..self }
    }

    /// Sets the bootstore path for the [`crate::Discv5Driver`].
    pub fn with_bootstore(self, bootstore: Option<PathBuf>) -> Self {
        if let Some(bootstore) = bootstore {
            return Self { discovery: self.discovery.with_bootstore(bootstore), ..self };
        }
        self
    }

    pub fn with_bootnodes(self, bootnodes: Vec<Enr>) -> Self {
        Self { discovery: self.discovery.with_bootnodes(bootnodes), ..self }
    }

    /// Sets the block time used by peer scoring.
    pub fn with_block_time(self, block_time: u64) -> Self {
        Self { gossip: self.gossip.with_block_time(block_time), ..self }
    }

    /// Sets the peer scoring based on the given [`PeerScoreLevel`].
    pub fn with_peer_scoring(self, level: PeerScoreLevel) -> Self {
        Self { gossip: self.gossip.with_peer_scoring(level), ..self }
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
    pub fn with_discovery_address(self, address: AdvertisedIpAndPort) -> Self {
        Self { discovery: self.discovery.with_address(address), ..self }
    }

    /// Sets the chain ID for both the [`crate::Discv5Driver`] and [`crate::GossipDriver`].
    pub fn with_chain_id(self, id: u64) -> Self {
        Self {
            gossip: self.gossip.with_chain_id(id),
            discovery: self.discovery.with_chain_id(id),
            ..self
        }
    }

    /// Sets the gossipsub config for the [`crate::GossipDriver`].
    pub fn with_gossip_config(self, config: libp2p::gossipsub::Config) -> Self {
        Self { gossip: self.gossip.with_config(config), ..self }
    }

    /// Sets the publish receiver for the [`crate::Network`].
    pub fn with_publish_receiver(
        self,
        publish_rx: tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>,
    ) -> Self {
        Self { publish_rx: Some(publish_rx), ..self }
    }

    /// Sets the [`RollupConfig`] for the [`crate::Network`].
    pub fn with_rollup_config(self, cfg: RollupConfig) -> Self {
        Self { cfg: Some(cfg), ..self }
    }

    /// Sets the rpc receiver for the [`crate::Network`].
    pub fn with_rpc_receiver(self, rpc_recv: tokio::sync::mpsc::Receiver<NetRpcRequest>) -> Self {
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

    /// Sets the keypair for the [`crate::GossipDriver`].
    pub fn with_keypair(self, keypair: Keypair) -> Self {
        Self { gossip: self.gossip.with_keypair(keypair), ..self }
    }

    /// Sets the unsafe block signer for the [`crate::GossipDriver`].
    pub fn with_unsafe_block_signer(self, signer: Address) -> Self {
        Self { signer: Some(signer), ..self }
    }

    /// Sets the unsafe block sender for the [`crate::Network`].
    pub fn with_unsafe_block_sender(
        self,
        sender: BroadcastSender<OpNetworkPayloadEnvelope>,
    ) -> Self {
        Self { payload_tx: Some(sender), ..self }
    }

    /// Builds the [`Network`].
    pub fn build(mut self) -> Result<Network, NetworkBuilderError> {
        let signer = self.signer.take().ok_or(NetworkBuilderError::UnsafeBlockSignerNotSet)?;
        let (signer_tx, signer_rx) = tokio::sync::watch::channel(signer);
        let unsafe_block_signer_sender = Some(signer_tx);
        let gossip = self.gossip.with_unsafe_block_signer_receiver(signer_rx).build()?;
        let discovery = self.discovery.build()?;
        let rpc = self.rpc_recv.take();
        let payload_tx = self.payload_tx.unwrap_or(tokio::sync::broadcast::channel(256).0);

        let publish_rx = self.publish_rx.take();
        let cfg = self.cfg.take();

        Ok(Network {
            gossip,
            discovery,
            unsafe_block_signer_sender,
            rpc,
            broadcast: Broadcast::new(payload_tx),
            publish_rx,
            cfg,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GossipDriverBuilderError;
    use discv5::{ConfigBuilder, ListenConfig};
    use libp2p::gossipsub::IdentTopic;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_build_missing_unsafe_block_signer() {
        let builder = NetworkBuilder::new();
        let err = builder.build().unwrap_err();
        assert_eq!(err, NetworkBuilderError::UnsafeBlockSignerNotSet);
    }

    #[test]
    fn test_build_missing_chain_id() {
        let builder = NetworkBuilder::new();
        let err = builder
            .with_unsafe_block_signer(Address::random())
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .build()
            .unwrap_err();
        let gossip_err = GossipDriverBuilderError::MissingChainID;
        assert_eq!(err, NetworkBuilderError::GossipDriverBuilder(gossip_err));
    }

    #[test]
    fn test_build_missing_gossip_address() {
        let builder = NetworkBuilder::new();
        let err = builder
            .with_unsafe_block_signer(Address::random())
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .with_chain_id(1)
            .build()
            .unwrap_err();
        let gossip_err = GossipDriverBuilderError::GossipAddrNotSet;
        assert_eq!(err, NetworkBuilderError::GossipDriverBuilder(gossip_err));
    }

    #[test]
    fn test_build_simple_succeeds() {
        let id = 10;
        let signer = Address::random();
        let disc_listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9097);
        let disc_enr = AdvertisedIpAndPort::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098, 9098);
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));
        let driver = NetworkBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .with_gossip_address(gossip_addr.clone())
            .with_discovery_address(disc_enr)
            .with_discovery_config(ConfigBuilder::new(disc_listen.into()).build())
            .build()
            .unwrap();

        // Driver Assertions
        assert_eq!(driver.gossip.addr, gossip_addr);
        assert_eq!(driver.discovery.chain_id, id);
        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9098);

        // Block Handler Assertions
        assert_eq!(driver.gossip.handler.chain_id, id);
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
        let id = 10;
        let signer = Address::random();
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));
        let disc = AdvertisedIpAndPort::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9097, 9097);
        let discovery_config =
            ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098))
                .build();
        let driver = NetworkBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_address(gossip_addr)
            .with_discovery_address(disc)
            .with_discovery_config(discovery_config)
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .build()
            .unwrap();

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9097);
    }
}
