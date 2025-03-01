#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
pub use alloy_primitives::map::{DefaultHashBuilder, HashMap};
pub use kona_genesis::{ChainConfig, RollupConfig};

pub mod chain_list;
pub use chain_list::{Chain, ChainList};

pub mod superchain;
pub use superchain::Registry;

#[cfg(test)]
pub mod test_utils;

lazy_static::lazy_static! {
    /// Private initializer that loads the superchain configurations.
    static ref _INIT: Registry = Registry::from_chain_list();

    /// Chain configurations exported from the registry
    pub static ref CHAINS: ChainList = _INIT.chain_list.clone();

    /// OP Chain configurations exported from the registry
    pub static ref OPCHAINS: HashMap<u64, ChainConfig, DefaultHashBuilder> = _INIT.op_chains.clone();

    /// Rollup configurations exported from the registry
    pub static ref ROLLUP_CONFIGS: HashMap<u64, RollupConfig, DefaultHashBuilder> = _INIT.rollup_configs.clone();
}

/// Returns all available [Chain] identifiers.
pub fn chain_idents() -> Vec<String> {
    CHAINS.chains.iter().map(|c| c.identifier.clone()).collect()
}

/// Returns a [Chain] by its identifier.
pub fn chain_by_ident(ident: &str) -> Option<&Chain> {
    CHAINS.get_chain_by_ident(ident)
}

/// Returns a [RollupConfig] by its identifier.
pub fn rollup_config_by_ident(ident: &str) -> Option<&RollupConfig> {
    let chain_id = chain_by_ident(ident)?.chain_id;
    ROLLUP_CONFIGS.get(&chain_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardcoded_rollup_configs() {
        let test_cases = [
            (10, test_utils::OP_MAINNET_CONFIG),
            (8453, test_utils::BASE_MAINNET_CONFIG),
            (11155420, test_utils::OP_SEPOLIA_CONFIG),
            (84532, test_utils::BASE_SEPOLIA_CONFIG),
        ]
        .to_vec();

        for (chain_id, expected) in test_cases {
            let derived = super::ROLLUP_CONFIGS.get(&chain_id).unwrap();
            assert_eq!(expected, *derived);
        }
    }

    #[test]
    fn test_chain_by_ident() {
        let chain_by_ident = chain_by_ident("mainnet/base").unwrap();
        let chain_by_id = CHAINS.get_chain_by_id(8453).unwrap();
        assert_eq!(chain_by_ident, chain_by_id);
    }

    #[test]
    fn test_rollup_config_by_ident() {
        let rollup_config_by_ident = rollup_config_by_ident("mainnet/base").unwrap();
        let rollup_config_by_id = ROLLUP_CONFIGS.get(&8453).unwrap();
        assert_eq!(rollup_config_by_ident, rollup_config_by_id);
    }
}
