//! Network Builder Module.

use alloy_primitives::Address;
use discv5::{Config as Discv5Config, ListenConfig};
use kona_genesis::RollupConfig;
use libp2p::{Multiaddr, identity::Keypair};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::{net::SocketAddr, time::Duration};

use crate::{
    Config, Discv5Builder, GossipDriverBuilder, NetRpcRequest, Network, NetworkBuilderError,
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
    /// A receiver for unsafe blocks to publish.
    publish_rx: Option<tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>>,
}

impl From<Config> for NetworkBuilder {
    fn from(config: Config) -> Self {
        Self::new()
            .with_discovery_address(config.discovery_address)
            .with_gossip_address(config.gossip_address)
            .with_unsafe_block_signer(config.unsafe_block_signer)
            .with_keypair(config.keypair)
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
            publish_rx: None,
            cfg: None,
        }
    }

    /// Sets the address for the [`crate::Discv5Driver`].
    pub fn with_discovery_address(self, address: SocketAddr) -> Self {
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

    /// Sets the listen config for the [`crate::Discv5Driver`].
    pub fn with_listen_config(self, config: ListenConfig) -> Self {
        Self { discovery: self.discovery.with_listen_config(config), ..self }
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

    /// Builds the [`Network`].
    pub fn build(mut self) -> Result<Network, NetworkBuilderError> {
        let signer = self.signer.take().ok_or(NetworkBuilderError::UnsafeBlockSignerNotSet)?;
        let (signer_tx, signer_rx) = tokio::sync::watch::channel(signer);
        let unsafe_block_signer_sender = Some(signer_tx);
        let mut gossip = self.gossip.with_unsafe_block_signer_receiver(signer_rx).build()?;
        let discovery = self.discovery.build()?;
        let unsafe_block_recv = gossip.take_payload_recv();
        let rpc = self.rpc_recv.take();
        let publish_rx = self.publish_rx.take();
        let cfg = self.cfg.take();

        Ok(Network {
            gossip,
            discovery,
            unsafe_block_recv,
            unsafe_block_signer_sender,
            rpc,
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
        let disc = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098);
        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));
        let driver = NetworkBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_rpc_receiver(tokio::sync::mpsc::channel(1).1)
            .with_gossip_address(gossip_addr.clone())
            .with_discovery_address(disc)
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
        let disc = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9097);
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

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9098);
    }
}
