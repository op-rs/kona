//! Configuration for the `Network`.

use crate::{discv5::LocalNode, gossip::GaterConfig};
use alloy_primitives::Address;
use alloy_signer_local::PrivateKeySigner;
use discv5::Enr;
use kona_genesis::RollupConfig;
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use libp2p::identity::Keypair;
use std::path::PathBuf;
use tokio::time::Duration;

/// Configuration for kona's P2P stack.
#[derive(Debug, Clone)]
pub struct Config {
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
    /// The L2 Block Time.
    pub block_time: u64,
    /// An optional path to the bootstore.
    pub bootstore: Option<PathBuf>,
    /// The configuration for the connection gater.
    pub gater_config: GaterConfig,
    /// An optional list of bootnode ENRs to start the node with.
    pub bootnodes: Vec<Enr>,
    /// The [`RollupConfig`].
    pub rollup_config: RollupConfig,
    /// A local signer for payloads.
    pub local_signer: Option<PrivateKeySigner>,
}
