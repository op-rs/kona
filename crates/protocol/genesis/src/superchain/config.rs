//! Contains the `SuperchainConfig` type.

use crate::{HardForkConfiguration, SuperchainL1Info};
use alloc::string::String;
use alloy_primitives::Address;

/// A superchain configuration file format
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuperchainConfig {
    /// Superchain name (e.g. "Mainnet")
    pub name: String,
    /// Superchain L1 anchor information
    pub l1: SuperchainL1Info,
    /// Default hardforks timestamps.
    pub hardforks: HardForkConfiguration,
    /// Optional addresses for the superchain-wide default protocol versions contract.
    #[cfg_attr(feature = "serde", serde(alias = "protocolVersionsAddr"))]
    pub protocol_versions_addr: Option<Address>,
    /// Optional address for the superchain-wide default superchain config contract.
    #[cfg_attr(feature = "serde", serde(alias = "superchainConfigAddr"))]
    pub superchain_config_addr: Option<Address>,
    /// The op contracts manager proxy address.
    #[cfg_attr(feature = "serde", serde(alias = "OPContractsManagerProxyAddr"))]
    pub op_contracts_manager_proxy_addr: Option<Address>,
}
