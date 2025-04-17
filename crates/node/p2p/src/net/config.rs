//! Configuration for the `Network`.

use crate::{PeerScoreLevel, peers::PeerMonitoring};
use alloy_primitives::Address;
use libp2p::identity::Keypair;
use std::{net::SocketAddr, path::PathBuf};
use tokio::time::Duration;

/// Configuration for kona's P2P stack.
#[derive(Debug, Clone)]
pub struct Config {
    /// Discovery Config.
    pub discovery_config: discv5::Config,
    /// The discovery address.
    pub discovery_address: SocketAddr,
    /// The interval to find peers.
    pub discovery_interval: Duration,
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
    /// Peer score monitoring config.
    pub monitor_peers: Option<PeerMonitoring>,
    /// The L2 Block Time.
    pub block_time: u64,
    /// An optional path to the bootstore.
    pub bootstore: Option<PathBuf>,
}
