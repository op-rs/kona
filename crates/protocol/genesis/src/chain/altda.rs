//! Contains the AltDA config type.

use alloc::string::String;
use alloy_primitives::Address;

/// AltDA configuration.
///
/// See: <https://github.com/ethereum-optimism/superchain-registry/blob/8ff62ada16e14dd59d0fb94ffb47761c7fa96e01/ops/internal/config/chain.go#L133-L138>
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AltDAConfig {
    /// AltDA challenge address
    #[cfg_attr(feature = "serde", serde(alias = "da_challenge_contract_address"))]
    pub da_challenge_address: Option<Address>,
    /// AltDA challenge window time (in seconds)
    pub da_challenge_window: Option<u64>,
    /// AltDA resolution window time (in seconds)
    pub da_resolve_window: Option<u64>,
    /// AltDA commitment type
    pub da_commitment_type: Option<String>,
}
