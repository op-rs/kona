//! A builder for the [`GossipDriver`].

use alloy_primitives::Address;
use libp2p::{
    Multiaddr, SwarmBuilder, gossipsub::Config, identity::Keypair, noise::Config as NoiseConfig,
    tcp::Config as TcpConfig, yamux::Config as YamuxConfig,
};
use std::time::Duration;
use tokio::sync::watch::Receiver;

use crate::{
    Behaviour, BlockHandler, GossipDriver, GossipDriverBuilderError, PeerScoreLevel,
    peers::PeerMonitoring,
};

/// A builder for the [`GossipDriver`].
#[derive(Debug, Default)]
pub struct GossipDriverBuilder {
    /// The chain id of the network.
    chain_id: Option<u64>,
    /// The idle connection timeout as a [`Duration`].
    timeout: Option<Duration>,
    /// The [`Keypair`] for the node.
    keypair: Option<Keypair>,
    /// The [`Multiaddr`] for the gossip driver to listen on.
    gossip_addr: Option<Multiaddr>,
    /// Unsafe block signer [`Receiver`].
    signer: Option<Receiver<Address>>,
    /// Sets the [`PeerScoreLevel`] for the [`Behaviour`].
    scoring: Option<PeerScoreLevel>,
    /// The [`Config`] for the [`Behaviour`].
    config: Option<Config>,
    /// Sets the block time for the peer scoring.
    block_time: Option<u64>,
    /// If set, the gossip layer will monitor peer scores and ban peers that are below a given
    /// threshold.
    peer_monitoring: Option<PeerMonitoring>,
    /// The number of times to redial a peer.
    /// If unset, peers will not be redialed.
    /// If set to `0`, peers will be redialed indefinitely.
    peer_redial: Option<u64>,
}

impl GossipDriverBuilder {
    /// Creates a new [`GossipDriverBuilder`].
    pub const fn new() -> Self {
        Self {
            chain_id: None,
            timeout: None,
            keypair: None,
            gossip_addr: None,
            signer: None,
            scoring: None,
            config: None,
            block_time: None,
            peer_monitoring: None,
            peer_redial: None,
        }
    }

    /// Sets the number of times to redial a peer.
    /// If unset, peers will not be redialed.
    /// If set to `0`, peers will be redialed indefinitely.
    pub const fn with_peer_redial(mut self, peer_redial: Option<u64>) -> Self {
        self.peer_redial = peer_redial;
        self
    }

    /// Specifies the chain ID of the gossip driver.
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Sets the block time for the peer scoring.
    pub const fn with_block_time(mut self, block_time: u64) -> Self {
        self.block_time = Some(block_time);
        self
    }

    /// Sets the [`PeerScoreLevel`] for the [`Behaviour`].
    pub const fn with_peer_scoring(mut self, level: PeerScoreLevel) -> Self {
        self.scoring = Some(level);
        self
    }

    /// Sets the [`PeerMonitoring`] configuration for the gossip driver.
    pub const fn with_peer_monitoring(mut self, peer_monitoring: Option<PeerMonitoring>) -> Self {
        self.peer_monitoring = peer_monitoring;
        self
    }

    /// Sets the unsafe block signer [`Address`] [`Receiver`] channel.
    pub fn with_unsafe_block_signer_receiver(mut self, signer: Receiver<Address>) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Sets the [`Keypair`] for the node.
    pub fn with_keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Sets the swarm's idle connection timeout.
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the [`Multiaddr`] for the gossip driver to listen on.
    pub fn with_address(mut self, addr: Multiaddr) -> Self {
        self.gossip_addr = Some(addr);
        self
    }

    /// Sets the [`Config`] for the [`Behaviour`].
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Builds the [`GossipDriver`].
    pub fn build(mut self) -> Result<GossipDriver, GossipDriverBuilderError> {
        // Extract builder arguments
        let timeout = self.timeout.take().unwrap_or(Duration::from_secs(60));
        let keypair = self.keypair.take().unwrap_or(Keypair::generate_secp256k1());
        let chain_id = self.chain_id.ok_or(GossipDriverBuilderError::MissingChainID)?;
        let addr = self.gossip_addr.take().ok_or(GossipDriverBuilderError::GossipAddrNotSet)?;
        let signer_recv = self.signer.ok_or(GossipDriverBuilderError::MissingUnsafeBlockSigner)?;

        // Block Handler setup
        let handler = BlockHandler::new(chain_id, signer_recv);

        // Construct the gossip behaviour
        let config = self.config.unwrap_or(crate::default_config());
        info!(
            "Config [Mesh D: {}] [Mesh L: {}] [Mesh H: {}] [Gossip Lazy: {}] [Flood Publish: {}]",
            config.mesh_n(),
            config.mesh_n_low(),
            config.mesh_n_high(),
            config.gossip_lazy(),
            config.flood_publish()
        );
        let mut behaviour = Behaviour::new(config, &[Box::new(handler.clone())])?;

        // If peer scoring is configured, set it on the behaviour.
        if let Some(scoring) = self.scoring {
            use crate::gossip::handler::Handler;
            let block_time = self.block_time.ok_or(GossipDriverBuilderError::MissingL2BlockTime)?;
            let params = scoring.to_params(handler.topics(), block_time).unwrap_or_default();
            match behaviour.gossipsub.with_peer_score(params, PeerScoreLevel::thresholds()) {
                Ok(_) => debug!(target: "scoring", "Peer scoring enabled successfully"),
                Err(e) => warn!(target: "scoring", "Peer scoring failed: {}", e),
            }
        } else {
            info!(target: "scoring", "Peer scoring not enabled");
        }

        // Build the swarm.
        info!("Building Swarm with Peer ID: {}", keypair.public().to_peer_id());
        let swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                TcpConfig::default(),
                |i: &Keypair| {
                    info!("Noise Config Peer ID: {}", i.public().to_peer_id());
                    NoiseConfig::new(i)
                },
                YamuxConfig::default,
            )
            .map_err(|_| GossipDriverBuilderError::TcpError)?
            .with_behaviour(|_| behaviour)
            .map_err(|_| GossipDriverBuilderError::WithBehaviourError)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(timeout))
            .build();

        let redialing = self.peer_redial;

        Ok(GossipDriver::new(swarm, addr, redialing, handler))
    }
}
