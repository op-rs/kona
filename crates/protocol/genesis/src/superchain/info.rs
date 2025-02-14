//! Contains the superchain L1 information.

use alloc::string::String;

/// Superchain L1 anchor information
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperchainL1Info {
    /// L1 chain ID
    #[cfg_attr(feature = "serde", serde(alias = "chainId"))]
    pub chain_id: u64,
    /// L1 chain public RPC endpoint
    #[cfg_attr(feature = "serde", serde(alias = "publicRPC"))]
    pub public_rpc: String,
    /// L1 chain explorer RPC endpoint
    pub explorer: String,
}
