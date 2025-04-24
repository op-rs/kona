//! Contains the runtime configuration for the node.

use alloy_primitives::Address;
use kona_rpc::ProtocolVersion;

/// The runtime config.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// The Unsafe Block Signer Address.
    /// This is also called the "p2p block signer address".
    pub unsafe_block_signer_address: Address,
    /// The required protocol version.
    pub required_protocol_version: ProtocolVersion,
    /// The recommended protocol version.
    pub recommended_protocol_version: ProtocolVersion,
}
