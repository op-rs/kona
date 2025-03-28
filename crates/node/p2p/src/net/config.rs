//! Configuration for the `Network`.

use alloy_primitives::Address;
use libp2p::identity::Keypair;
use std::net::SocketAddr;

/// Configuration for kona's P2P stack.
#[derive(Debug, Clone)]
pub struct Config {
    /// The discovery address.
    pub discovery_address: SocketAddr,
    /// The gossip address.
    pub gossip_address: libp2p::Multiaddr,
    /// The unsafe block signer.
    pub unsafe_block_signer: Address,
    /// The keypair.
    pub keypair: Keypair,
}
