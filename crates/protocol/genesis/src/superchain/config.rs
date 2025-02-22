//! Contains the `SuperchainConfig` type.

use crate::{HardForkConfig, SuperchainL1Info};
use alloc::string::String;
use alloy_primitives::Address;

/// A superchain configuration file format
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SuperchainConfig {
    /// Superchain name (e.g. "Mainnet")
    pub name: String,
    /// Superchain L1 anchor information
    pub l1: SuperchainL1Info,
    /// Default hardforks timestamps.
    pub hardforks: HardForkConfig,
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

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;
    use crate::{HardForkConfig, SuperchainL1Info};
    use alloc::string::ToString;

    #[test]
    fn test_deny_unknown_fields_sc_cfg() {
        let raw: &str = r#"
        {
            "name": "Mainnet",
            "l1": {
                "chainId": 10,
                "publicRPC": "https://mainnet.rpc",
                "explorer": "https://mainnet.explorer"
            },
            "hardforks": {
                "canyon_time": 1699981200,
                "delta_time": 1703203200,
                "ecotone_time": 1708534800,
                "fjord_time": 1716998400,
                "granite_time": 1723478400,
                "holocene_time": 1732633200
            },
            "unknown_field": "unknown"
        }
        "#;

        let err = serde_json::from_str::<SuperchainConfig>(raw).unwrap_err();
        assert_eq!(err.classify(), serde_json::error::Category::Data);
    }

    #[test]
    fn test_sc_cfg_serde() {
        let raw: &str = r#"
        {
            "name": "Mainnet",
            "l1": {
                "chainId": 10,
                "publicRPC": "https://mainnet.rpc",
                "explorer": "https://mainnet.explorer"
            },
            "hardforks": {
                "canyon_time": 1699981200,
                "delta_time": 1703203200,
                "ecotone_time": 1708534800,
                "fjord_time": 1716998400,
                "granite_time": 1723478400,
                "holocene_time": 1732633200
            }
        }
        "#;

        let config = SuperchainConfig {
            name: "Mainnet".to_string(),
            l1: SuperchainL1Info {
                chain_id: 10,
                public_rpc: "https://mainnet.rpc".to_string(),
                explorer: "https://mainnet.explorer".to_string(),
            },
            hardforks: HardForkConfig {
                canyon_time: Some(1699981200),
                delta_time: Some(1703203200),
                ecotone_time: Some(1708534800),
                fjord_time: Some(1716998400),
                granite_time: Some(1723478400),
                holocene_time: Some(1732633200),
                isthmus_time: None,
                interop_time: None,
            },
            protocol_versions_addr: None,
            superchain_config_addr: None,
            op_contracts_manager_proxy_addr: None,
        };

        let deserialized: SuperchainConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(config, deserialized);
    }
}
