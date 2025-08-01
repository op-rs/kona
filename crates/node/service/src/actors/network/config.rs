//! Configuration for the `Network`.

use alloy_primitives::Address;
use discv5::Enr;
use kona_genesis::RollupConfig;
use kona_p2p::{GaterConfig, LocalNode};
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use kona_sources::BlockSigner;
use libp2p::{Multiaddr, identity::Keypair};
use std::path::PathBuf;
use tokio::time::Duration;

/// Configuration for kona's P2P stack.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Discovery Config.
    pub discovery_config: discv5::Config,
    /// The local node's advertised address to external peers.
    /// Note: This may be different from the node's discovery listen address.
    pub discovery_address: LocalNode,
    /// The interval to find peers.
    pub discovery_interval: Duration,
    /// The interval to remove peers from the discovery service.
    pub discovery_randomize: Option<Duration>,
    /// The gossip address.
    pub gossip_address: libp2p::Multiaddr,
    /// The unsafe block signer.
    pub unsafe_block_signer: Address,
    /// The keypair.
    pub keypair: Keypair,
    /// The gossip config.
    pub gossip_config: libp2p::gossipsub::Config,
    /// The peer score level.
    pub scoring: PeerScoreLevel,
    /// Whether to enable topic scoring.
    pub topic_scoring: bool,
    /// Peer score monitoring config.
    pub monitor_peers: Option<PeerMonitoring>,
    /// An optional path to the bootstore.
    pub bootstore: Option<PathBuf>,
    /// The configuration for the connection gater.
    pub gater_config: GaterConfig,
    /// An optional list of bootnode ENRs to start the node with.
    pub bootnodes: Vec<Enr>,
    /// The [`RollupConfig`].
    pub rollup_config: RollupConfig,
    /// A signer for gossip payloads.
    pub gossip_signer: Option<BlockSigner>,
}

impl NetworkConfig {
    const DEFAULT_DISCOVERY_INTERVAL: Duration = Duration::from_secs(5);
    const DEFAULT_DISCOVERY_RANDOMIZE: Option<Duration> = None;

    /// Creates a new [`NetworkConfig`] with the given [`RollupConfig`] with the minimum required
    /// fields. Generates a random keypair for the node.
    pub fn new(
        rollup_config: RollupConfig,
        discovery_listen: LocalNode,
        gossip_address: Multiaddr,
        unsafe_block_signer: Address,
    ) -> Self {
        Self {
            rollup_config,
            discovery_config: discv5::ConfigBuilder::new((&discovery_listen).into()).build(),
            discovery_address: discovery_listen,
            discovery_interval: Self::DEFAULT_DISCOVERY_INTERVAL,
            discovery_randomize: Self::DEFAULT_DISCOVERY_RANDOMIZE,
            gossip_address,
            unsafe_block_signer,
            keypair: Keypair::generate_secp256k1(),
            bootnodes: Default::default(),
            bootstore: Default::default(),
            gater_config: Default::default(),
            gossip_config: Default::default(),
            scoring: Default::default(),
            topic_scoring: Default::default(),
            monitor_peers: Default::default(),
            gossip_signer: Default::default(),
        }
    }
}
