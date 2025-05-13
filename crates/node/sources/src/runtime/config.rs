//! Contains the runtime configuration for the node.

use alloy_primitives::Address;
use op_alloy_rpc_types_engine::ProtocolVersion;

/// The runtime config.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// The Unsafe Block Signer Address.
    /// This is also called the "p2p block signer address".
    pub unsafe_block_signer_address: Address,
    /// The required protocol version.
    pub required_protocol_version: ProtocolVersion,
    /// The recommended protocol version.
    pub recommended_protocol_version: ProtocolVersion,
}

impl std::fmt::Display for RuntimeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RuntimeConfig {{ unsafe_block_signer_address: {}, required_protocol_version: {}, recommended_protocol_version: {} }}",
            self.unsafe_block_signer_address,
            self.required_protocol_version,
            self.recommended_protocol_version
        )
    }
}
