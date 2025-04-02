//! A builder for the [`GossipDriver`].

use alloy_primitives::Address;
use libp2p::{
    Multiaddr, SwarmBuilder, identity::Keypair, noise::Config as NoiseConfig,
    tcp::Config as TcpConfig, yamux::Config as YamuxConfig,
};
use std::time::Duration;
use tokio::sync::watch::Receiver;

use crate::{Behaviour, BlockHandler, GossipDriver, GossipDriverBuilderError};

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
}

impl GossipDriverBuilder {
    /// Creates a new [`GossipDriverBuilder`].
    pub const fn new() -> Self {
        Self { chain_id: None, timeout: None, keypair: None, gossip_addr: None, signer: None }
    }

    /// Specifies the chain ID of the gossip driver.
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
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
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the [`Multiaddr`] for the gossip driver to listen on.
    pub fn with_address(mut self, addr: Multiaddr) -> Self {
        self.gossip_addr = Some(addr);
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
        let config = crate::default_config();
        let behaviour = Behaviour::new(config, &[Box::new(handler.clone())])?;

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

        Ok(GossipDriver::new(swarm, addr, handler.clone()))
    }
}
