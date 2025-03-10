//! Network Builder Module.

use alloy_primitives::Address;
use discv5::{Config, ListenConfig};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::sync::watch::channel;

use libp2p::{
    Multiaddr, SwarmBuilder, gossipsub::Config as GossipConfig, multiaddr::Protocol,
    noise::Config as NoiseConfig, tcp::Config as TcpConfig, yamux::Config as YamuxConfig,
};
use libp2p_identity::Keypair;

use crate::{
    discovery::builder::{DiscoveryBuilder, DiscoveryBuilderError},
    driver::NetworkDriver,
    gossip::{
        behaviour::{Behaviour, BehaviourError},
        config,
        driver::GossipDriver,
        handler::BlockHandler,
    },
};

/// An error from the [NetworkDriverBuilder].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkDriverBuilderError {
    /// The unsafe block signer is not set.
    #[error("unsafe block signer not set")]
    UnsafeBlockSignerNotSet,
    /// The chain ID is not set.
    #[error("chain ID not set")]
    ChainIdNotSet,
    /// The gossip address is not set.
    #[error("gossip_addr address not set")]
    GossipAddrNotSet,
    /// A behaviour error.
    #[error("behaviour error: {0}")]
    BehaviourError(#[from] BehaviourError),
    /// A config builder error.
    #[error("config builder error")]
    ConfigBuilderError,
    /// A TCP error.
    #[error("TCP error")]
    TcpError,
    /// An error when setting the behaviour on the swarm builder.
    #[error("error setting behaviour on swarm builder")]
    WithBehaviourError,
    /// A discovery builder error.
    #[error("discovery builder error: {0}")]
    DiscoveryBuilderError(#[from] DiscoveryBuilderError),
}

/// Constructs a [NetworkDriver] for Optimism's consensus-layer.
#[derive(Default)]
pub struct NetworkDriverBuilder {
    /// The chain ID of the network.
    pub chain_id: Option<u64>,
    /// The unsafe block signer.
    pub unsafe_block_signer: Option<Address>,
    /// The socket address that the gossip service is listening on.
    pub gossip_addr: Option<SocketAddr>,
    /// The listen config that the discovery service is listening on.
    pub discovery_addr: Option<ListenConfig>,
    /// The [GossipConfig] constructs the config for `gossipsub`.
    pub gossip_config: Option<GossipConfig>,
    /// The interval to discovery random nodes.
    pub interval: Option<Duration>,
    /// The [Config] constructs the config for `discv5`.
    pub discovery_config: Option<Config>,
    /// The [Keypair] for the node.
    pub keypair: Option<Keypair>,
    /// The [TcpConfig] for the swarm.
    pub tcp_config: Option<TcpConfig>,
    /// The [NoiseConfig] for the swarm.
    pub noise_config: Option<NoiseConfig>,
    /// The [YamuxConfig] for the swarm.
    pub yamux_config: Option<YamuxConfig>,
    /// The idle connection timeout.
    pub timeout: Option<Duration>,
}

impl NetworkDriverBuilder {
    /// Creates a new [NetworkDriverBuilder].
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies the chain ID of the network.
    pub fn with_chain_id(&mut self, chain_id: u64) -> &mut Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Specifies the unsafe block signer.
    pub fn with_unsafe_block_signer(&mut self, unsafe_block_signer: Address) -> &mut Self {
        self.unsafe_block_signer = Some(unsafe_block_signer);
        self
    }

    /// Specifies the interval to discovery random nodes.
    pub fn with_interval(&mut self, interval: Duration) -> &mut Self {
        self.interval = Some(interval);
        self
    }

    /// Specifies the socket address that the gossip service is listening on.
    pub fn with_gossip_addr(&mut self, socket: SocketAddr) -> &mut Self {
        self.gossip_addr = Some(socket);
        self
    }

    /// Specifies the listen config that the discovery service is listening on.
    pub fn with_discovery_addr(&mut self, listen_config: ListenConfig) -> &mut Self {
        self.discovery_addr = Some(listen_config);
        self
    }

    /// Specifies the keypair for the node.
    pub fn with_keypair(&mut self, keypair: Keypair) -> &mut Self {
        self.keypair = Some(keypair);
        self
    }

    /// Specifies the [TcpConfig] for the swarm.
    pub fn with_tcp_config(&mut self, tcp_config: TcpConfig) -> &mut Self {
        self.tcp_config = Some(tcp_config);
        self
    }

    /// Specifies the [NoiseConfig] for the swarm.
    pub fn with_noise_config(&mut self, noise_config: NoiseConfig) -> &mut Self {
        self.noise_config = Some(noise_config);
        self
    }

    /// Specifies the [YamuxConfig] for the swarm.
    pub fn with_yamux_config(&mut self, yamux_config: YamuxConfig) -> &mut Self {
        self.yamux_config = Some(yamux_config);
        self
    }

    /// Set the swarm's idle connection timeout.
    pub fn with_idle_connection_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = Some(timeout);
        self
    }

    /// Specifies the [GossipConfig] for the `gossipsub` configuration.
    ///
    /// If not set, the [NetworkDriverBuilder] will use the default gossipsub
    /// configuration defined in [config::default_config]. These defaults can
    /// be extended by using the [config::default_config_builder] method to
    /// build a custom [GossipConfig].
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use kona_p2p::gossip::config;
    /// use kona_p2p::NetworkDriverBuilder;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let chain_id = 10;
    /// let signer = Address::random();
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    ///
    /// // Let's say we want to enable flood publishing and use all other default settings.
    /// let cfg = config::default_config_builder().flood_publish(true).build().unwrap();
    /// let mut builder = NetworkDriverBuilder::new()
    ///    .with_unsafe_block_signer(signer)
    ///    .with_chain_id(chain_id)
    ///    .with_gossip_addr(socket)
    ///    .with_gossip_config(cfg);
    ///    .build()
    ///    .unwrap();
    /// ```
    pub fn with_gossip_config(&mut self, cfg: GossipConfig) -> &mut Self {
        self.gossip_config = Some(cfg);
        self
    }

    /// Specifies the [Config] for the `discv5` configuration.
    ///
    /// If not set, the [NetworkDriverBuilder] will fall back to use the [ListenConfig]
    /// to construct [Config]. These defaults can be extended by using the
    /// [discv5::ConfigBuilder::new] method to build a custom [Config].
    ///
    /// ## Example
    ///
    /// ```rust
    /// use alloy_primitives::{Address, address};
    /// use discv5::{ConfigBuilder, ListenConfig};
    /// use kona_p2p::builder::NetworkDriverBuilder;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let id = 10;
    /// let signer = Address::random();
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    /// let discovery_config =
    ///     ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098))
    ///         .build();
    /// let driver = NetworkDriverBuilder::new()
    ///     .with_unsafe_block_signer(signer)
    ///     .with_chain_id(id)
    ///     .with_gossip_addr(socket)
    ///     .with_discovery_config(discovery_config)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn with_discovery_config(&mut self, cfg: Config) -> &mut Self {
        self.discovery_config = Some(cfg);
        self
    }

    /// Builds the [NetworkDriver].
    ///
    /// ## Errors
    ///
    /// Returns an error if any of the following required fields are not set:
    /// - [NetworkDriverBuilder::unsafe_block_signer]
    /// - [NetworkDriverBuilder::chain_id]
    /// - [NetworkDriverBuilder::gossip_addr]
    ///
    /// If explicitly set, the following fields are used for discovery address, otherwise the gossip
    /// address is used:
    /// - [NetworkDriverBuilder::discovery_addr]
    ///
    /// Set these fields using the respective methods on the [NetworkDriverBuilder]
    /// before calling this method.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use kona_p2p::NetworkDriverBuilder;
    ///
    /// let chain_id = 10;
    /// let signer = Address::random();
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    /// let driver = NetworkDriverBuilder::new()
    ///    .with_unsafe_block_signer(signer)
    ///    .with_chain_id(chain_id)
    ///    .with_gossip_addr(socket)
    ///    .build()
    ///    .unwrap();
    ///
    /// Or if you want to use a different discovery address:
    ///
    /// let chain_id = 10;
    /// let signer = Address::random();
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    /// let listen_config = ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9999);
    /// let driver = NetworkDriverBuilder::new()
    ///    .with_unsafe_block_signer(signer)
    ///    .with_chain_id(chain_id)
    ///    .with_gossip_addr(socket)
    ///    .with_discovery_addr(listen_config)
    ///    .build()
    ///    .unwrap();
    /// ```
    pub fn build(&mut self) -> Result<NetworkDriver, NetworkDriverBuilderError> {
        // Build the config for gossipsub.
        let config = match self.gossip_config.take() {
            Some(cfg) => cfg,
            None => config::default_config()
                .map_err(|_| NetworkDriverBuilderError::ConfigBuilderError)?,
        };
        let unsafe_block_signer =
            self.unsafe_block_signer.ok_or(NetworkDriverBuilderError::UnsafeBlockSignerNotSet)?;
        let chain_id = self.chain_id.ok_or(NetworkDriverBuilderError::ChainIdNotSet)?;

        // Create the block handler.
        let (unsafe_block_signer_sender, unsafe_block_signer_recv) = channel(unsafe_block_signer);
        let (handler, unsafe_block_recv) = BlockHandler::new(chain_id, unsafe_block_signer_recv);

        // Construct the gossipsub behaviour.
        let behaviour = Behaviour::new(config, &[Box::new(handler.clone())])?;

        // Build the swarm.
        let timeout = self.timeout.take().unwrap_or(Duration::from_secs(60));
        let noise_config = self.noise_config.take();
        let keypair = self.keypair.take().unwrap_or(Keypair::generate_secp256k1());
        let swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                self.tcp_config.take().unwrap_or_default(),
                |i: &Keypair| match noise_config {
                    Some(cfg) => Ok(cfg),
                    None => NoiseConfig::new(i),
                },
                || self.yamux_config.take().unwrap_or_default(),
            )
            .map_err(|_| NetworkDriverBuilderError::TcpError)?
            .with_behaviour(|_| behaviour)
            .map_err(|_| NetworkDriverBuilderError::WithBehaviourError)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(timeout))
            .build();

        let gossip_addr =
            self.gossip_addr.take().ok_or(NetworkDriverBuilderError::GossipAddrNotSet)?;
        let mut multiaddr = Multiaddr::empty();
        match gossip_addr.ip() {
            IpAddr::V4(ip) => multiaddr.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multiaddr.push(Protocol::Ip6(ip)),
        }
        multiaddr.push(Protocol::Tcp(gossip_addr.port()));
        let gossip = GossipDriver::new(swarm, multiaddr, handler.clone());

        // Build the discovery service
        let mut discovery_builder =
            DiscoveryBuilder::new().with_address(gossip_addr).with_chain_id(chain_id);

        if let Some(discovery_addr) = self.discovery_addr.take() {
            discovery_builder = discovery_builder.with_listen_config(discovery_addr);
        }

        if let Some(discovery_config) = self.discovery_config.take() {
            discovery_builder = discovery_builder.with_discovery_config(discovery_config);
        }

        let mut discovery = discovery_builder.build()?;
        discovery.interval = self.interval.unwrap_or(Duration::from_secs(10));

        Ok(NetworkDriver {
            discovery,
            gossip,
            unsafe_block_recv: Some(unsafe_block_recv),
            unsafe_block_signer_sender: Some(unsafe_block_signer_sender),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use discv5::ConfigBuilder;
    use libp2p::gossipsub::IdentTopic;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_build_missing_unsafe_block_signer() {
        let mut builder = NetworkDriverBuilder::new();
        let Err(err) = builder.build() else {
            panic!("expected error when building NetworkDriver without unsafe block signer");
        };
        assert_eq!(err, NetworkDriverBuilderError::UnsafeBlockSignerNotSet);
    }

    #[test]
    fn test_build_missing_chain_id() {
        let mut builder = NetworkDriverBuilder::new();
        let Err(err) = builder.with_unsafe_block_signer(Address::random()).build() else {
            panic!("expected error when building NetworkDriver without chain id");
        };
        assert_eq!(err, NetworkDriverBuilderError::ChainIdNotSet);
    }

    #[test]
    fn test_build_missing_socket() {
        let mut builder = NetworkDriverBuilder::new();
        let Err(err) = builder.with_unsafe_block_signer(Address::random()).with_chain_id(1).build()
        else {
            panic!("expected error when building NetworkDriver without socket");
        };
        assert_eq!(err, NetworkDriverBuilderError::GossipAddrNotSet);
    }

    #[test]
    fn test_build_custom_gossip_config() {
        let id = 10;
        let signer = Address::random();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let cfg = config::default_config_builder().flood_publish(true).build().unwrap();
        let driver = NetworkDriverBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_addr(socket)
            .with_gossip_config(cfg)
            .build()
            .unwrap();
        let mut multiaddr = Multiaddr::empty();
        match socket.ip() {
            IpAddr::V4(ip) => multiaddr.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multiaddr.push(Protocol::Ip6(ip)),
        }
        multiaddr.push(Protocol::Tcp(socket.port()));

        // Driver Assertions
        assert_eq!(driver.gossip.addr, multiaddr);
        assert_eq!(driver.discovery.chain_id, id);

        // Block Handler Assertions
        assert_eq!(driver.gossip.handler.chain_id, id);
        let v1 = IdentTopic::new(format!("/optimism/{}/0/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v1_topic.hash(), v1.hash());
        let v2 = IdentTopic::new(format!("/optimism/{}/1/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v2_topic.hash(), v2.hash());
        let v3 = IdentTopic::new(format!("/optimism/{}/2/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v3_topic.hash(), v3.hash());
    }

    #[test]
    fn test_build_default_network_driver() {
        let id = 10;
        let signer = Address::random();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let driver = NetworkDriverBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_addr(socket)
            .build()
            .unwrap();
        let mut multiaddr = Multiaddr::empty();
        match socket.ip() {
            IpAddr::V4(ip) => multiaddr.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multiaddr.push(Protocol::Ip6(ip)),
        }
        multiaddr.push(Protocol::Tcp(socket.port()));

        // Driver Assertions
        assert_eq!(driver.gossip.addr, multiaddr);
        assert_eq!(driver.discovery.chain_id, id);
        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9099);

        // Block Handler Assertions
        assert_eq!(driver.gossip.handler.chain_id, id);
        let v1 = IdentTopic::new(format!("/optimism/{}/0/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v1_topic.hash(), v1.hash());
        let v2 = IdentTopic::new(format!("/optimism/{}/1/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v2_topic.hash(), v2.hash());
        let v3 = IdentTopic::new(format!("/optimism/{}/2/blocks", id));
        assert_eq!(driver.gossip.handler.blocks_v3_topic.hash(), v3.hash());
    }

    #[test]
    fn test_build_network_driver_with_discovery_addr() {
        let id = 10;
        let signer = Address::random();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery_addr = ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098);
        let driver = NetworkDriverBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_addr(socket)
            .with_discovery_addr(discovery_addr)
            .build()
            .unwrap();

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9098);
    }

    #[test]
    fn test_build_network_driver_with_discovery_config() {
        let id = 10;
        let signer = Address::random();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery_config =
            ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098))
                .build();
        let driver = NetworkDriverBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_addr(socket)
            .with_discovery_config(discovery_config)
            .build()
            .unwrap();

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9098);
    }

    #[test]
    fn test_build_network_driver_with_discovery_config_and_listen_config() {
        let id = 10;
        let signer = Address::random();
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery_config =
            ConfigBuilder::new(ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9098))
                .build();
        let discovery_addr = ListenConfig::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9097);
        let driver = NetworkDriverBuilder::new()
            .with_unsafe_block_signer(signer)
            .with_chain_id(id)
            .with_gossip_addr(socket)
            .with_discovery_addr(discovery_addr)
            .with_discovery_config(discovery_config)
            .build()
            .unwrap();

        assert_eq!(driver.discovery.disc.local_enr().tcp4().unwrap(), 9097);
    }
}
