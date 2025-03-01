//! Contains the full superchain data.

use super::ChainList;
use alloy_primitives::map::{DefaultHashBuilder, HashMap};
use kona_genesis::{ChainConfig, RollupConfig, Superchains};

/// The registry containing all the superchain configurations.
#[derive(Debug, Clone, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    /// The list of chains.
    pub chain_list: ChainList,
    /// Map of chain IDs to their chain configuration.
    pub op_chains: HashMap<u64, ChainConfig, DefaultHashBuilder>,
    /// Map of chain IDs to their rollup configurations.
    pub rollup_configs: HashMap<u64, RollupConfig, DefaultHashBuilder>,
}

impl Registry {
    /// Read the chain list.
    pub fn read_chain_list() -> ChainList {
        let chain_list = include_str!("../etc/chainList.json");
        serde_json::from_str(chain_list).expect("Failed to read chain list")
    }

    /// Read superchain configs.
    pub fn read_superchain_configs() -> Superchains {
        let superchain_configs = include_str!("../etc/configs.json");
        serde_json::from_str(superchain_configs).expect("Failed to read superchain configs")
    }

    /// Initialize the superchain configurations from the chain list.
    pub fn from_chain_list() -> Self {
        let chain_list = Self::read_chain_list();
        let superchains = Self::read_superchain_configs();
        let mut op_chains = HashMap::default();
        let mut rollup_configs = HashMap::default();

        for superchain in superchains.superchains {
            for mut chain_config in superchain.chains {
                chain_config.l1_chain_id = superchain.config.l1.chain_id;
                if let Some(a) = &mut chain_config.addresses {
                    a.zero_proof_addresses();
                }
                let mut rollup = chain_config.as_rollup_config();
                rollup.protocol_versions_address = superchain
                    .config
                    .protocol_versions_addr
                    .expect("Missing protocol versions address");
                rollup.superchain_config_address = superchain.config.superchain_config_addr;
                rollup_configs.insert(chain_config.chain_id, rollup);
                op_chains.insert(chain_config.chain_id, chain_config);
            }
        }

        Self { chain_list, op_chains, rollup_configs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloy_primitives::address;
    use kona_genesis::{AddressList, OP_MAINNET_BASE_FEE_CONFIG, Roles, SuperchainLevel};

    #[test]
    fn test_read_chain_configs() {
        let superchains = Registry::from_chain_list();
        assert!(superchains.chain_list.len() > 1);
        let base_config = ChainConfig {
            name: String::from("Base"),
            chain_id: 8453,
            l1_chain_id: 1,
            public_rpc: String::from("https://mainnet.base.org"),
            sequencer_rpc: String::from("https://mainnet-sequencer.base.org"),
            explorer: String::from("https://explorer.base.org"),
            superchain_level: SuperchainLevel::StandardCandidate,
            governed_by_optimism: false,
            superchain_time: Some(0),
            batch_inbox_addr: address!("ff00000000000000000000000000000000008453"),
            hardfork_config: crate::test_utils::BASE_MAINNET_CONFIG.hardforks,
            block_time: 2,
            seq_window_size: 3600,
            max_sequencer_drift: 600,
            data_availability_type: "eth-da".to_string(),
            optimism: Some(OP_MAINNET_BASE_FEE_CONFIG),
            alt_da: None,
            genesis: crate::test_utils::BASE_MAINNET_CONFIG.genesis,
            roles: Some(Roles {
                system_config_owner: Some(
                    "14536667Cd30e52C0b458BaACcB9faDA7046E056".parse().unwrap(),
                ),
                proxy_admin_owner: Some(
                    "7bB41C3008B3f03FE483B28b8DB90e19Cf07595c".parse().unwrap(),
                ),
                guardian: Some("09f7150d8c019bef34450d6920f6b3608cefdaf2".parse().unwrap()),
                challenger: Some("6F8C5bA3F59ea3E76300E3BEcDC231D656017824".parse().unwrap()),
                proposer: Some("642229f238fb9dE03374Be34B0eD8D9De80752c5".parse().unwrap()),
                unsafe_block_signer: Some(
                    "Af6E19BE0F9cE7f8afd49a1824851023A8249e8a".parse().unwrap(),
                ),
                batch_submitter: Some("5050F69a9786F081509234F1a7F4684b5E5b76C9".parse().unwrap()),
            }),
            addresses: Some(AddressList {
                address_manager: address!("8EfB6B5c4767B09Dc9AA6Af4eAA89F749522BaE2"),
                l1_cross_domain_messenger_proxy: address!(
                    "866E82a600A1414e583f7F13623F1aC5d58b0Afa"
                ),
                l1_erc721_bridge_proxy: address!("608d94945A64503E642E6370Ec598e519a2C1E53"),
                l1_standard_bridge_proxy: address!("3154Cf16ccdb4C6d922629664174b904d80F2C35"),
                l2_output_oracle_proxy: Some(address!("56315b90c40730925ec5485cf004d835058518A0")),
                optimism_mintable_erc20_factory_proxy: address!(
                    "05cc379EBD9B30BbA19C6fA282AB29218EC61D84"
                ),
                optimism_portal_proxy: address!("49048044D57e1C92A77f79988d21Fa8fAF74E97e"),
                system_config_proxy: address!("73a79Fab69143498Ed3712e519A88a918e1f4072"),
                proxy_admin: address!("0475cBCAebd9CE8AfA5025828d5b98DFb67E059E"),
                superchain_config: None,
                anchor_state_registry_proxy: Some(address!(
                    "db9091e48b1c42992a1213e6916184f9ebdbfedf"
                )),
                delayed_weth_proxy: Some(address!("a2f2ac6f5af72e494a227d79db20473cf7a1ffe8")),
                dispute_game_factory_proxy: Some(address!(
                    "43edb88c4b80fdd2adff2412a7bebf9df42cb40e"
                )),
                fault_dispute_game: Some(address!("cd3c0194db74c23807d4b90a5181e1b28cf7007c")),
                mips: Some(address!("16e83ce5ce29bf90ad9da06d2fe6a15d5f344ce4")),
                permissioned_dispute_game: Some(address!(
                    "19009debf8954b610f207d5925eede827805986e"
                )),
                preimage_oracle: Some(address!("9c065e11870b891d214bc2da7ef1f9ddfa1be277")),
                data_availability_challenge: None,
            }),
            gas_paying_token: None,
        };
        assert_eq!(*superchains.op_chains.get(&8453).unwrap(), base_config);
    }

    #[test]
    fn test_read_rollup_configs() {
        let superchains = Registry::from_chain_list();
        assert_eq!(
            *superchains.rollup_configs.get(&10).unwrap(),
            crate::test_utils::OP_MAINNET_CONFIG
        );
    }
}
