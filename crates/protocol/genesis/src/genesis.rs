//! Genesis types.

use alloy_eips::eip1898::BlockNumHash;

use crate::SystemConfig;

use alloy_primitives::{Address, address};

/// Container for all predeploy contract addresses
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Predeploys;

impl Predeploys {
    /// List of all predeploys.
    pub const ALL: [Address; 24] = [
        Self::LEGACY_MESSAGE_PASSER,
        Self::DEPLOYER_WHITELIST,
        Self::LEGACY_ERC20_ETH,
        Self::WETH9,
        Self::L2_CROSS_DOMAIN_MESSENGER,
        Self::L2_STANDARD_BRIDGE,
        Self::SEQUENCER_FEE_VAULT,
        Self::OP_MINTABLE_ERC20_FACTORY,
        Self::L1_BLOCK_NUMBER,
        Self::GAS_PRICE_ORACLE,
        Self::GOVERNANCE_TOKEN,
        Self::L1_BLOCK_INFO,
        Self::L2_TO_L1_MESSAGE_PASSER,
        Self::L2_ERC721_BRIDGE,
        Self::OP_MINTABLE_ERC721_FACTORY,
        Self::PROXY_ADMIN,
        Self::BASE_FEE_VAULT,
        Self::L1_FEE_VAULT,
        Self::SCHEMA_REGISTRY,
        Self::EAS,
        Self::BEACON_BLOCK_ROOT,
        Self::OPERATOR_FEE_VAULT,
        Self::CROSS_L2_INBOX,
        Self::L2_TO_L2_XDM,
    ];

    /// The LegacyMessagePasser contract stores commitments to withdrawal transactions before the
    /// Bedrock upgrade.
    pub const LEGACY_MESSAGE_PASSER: Address =
        address!("0x4200000000000000000000000000000000000000");

    /// The DeployerWhitelist was used to provide additional safety during initial phases of
    /// Optimism.
    pub const DEPLOYER_WHITELIST: Address = address!("0x4200000000000000000000000000000000000002");

    /// The LegacyERC20ETH predeploy represented all ether in the system before the Bedrock upgrade.
    pub const LEGACY_ERC20_ETH: Address = address!("0xDeadDeAddeAddEAddeadDEaDDEAdDeaDDeAD0000");

    /// The WETH9 predeploy address.
    pub const WETH9: Address = address!("0x4200000000000000000000000000000000000006");

    /// Higher level API for sending cross domain messages.
    pub const L2_CROSS_DOMAIN_MESSENGER: Address =
        address!("0x4200000000000000000000000000000000000007");

    /// The L2 cross-domain messenger proxy address.
    pub const L2_STANDARD_BRIDGE: Address = address!("0x4200000000000000000000000000000000000010");

    /// The sequencer fee vault proxy address.
    pub const SEQUENCER_FEE_VAULT: Address = address!("0x4200000000000000000000000000000000000011");

    /// The Optimism mintable ERC20 factory proxy address.
    pub const OP_MINTABLE_ERC20_FACTORY: Address =
        address!("0x4200000000000000000000000000000000000012");

    /// Returns the last known L1 block number (legacy system).
    pub const L1_BLOCK_NUMBER: Address = address!("0x4200000000000000000000000000000000000013");

    /// The gas price oracle proxy address.
    pub const GAS_PRICE_ORACLE: Address = address!("0x420000000000000000000000000000000000000F");

    /// The governance token proxy address.
    pub const GOVERNANCE_TOKEN: Address = address!("0x4200000000000000000000000000000000000042");

    /// The L1 block information proxy address.
    pub const L1_BLOCK_INFO: Address = address!("0x4200000000000000000000000000000000000015");

    /// The L2 contract `L2ToL1MessagePasser`, stores commitments to withdrawal transactions.
    pub const L2_TO_L1_MESSAGE_PASSER: Address =
        address!("0x4200000000000000000000000000000000000016");

    /// The L2 ERC721 bridge proxy address.
    pub const L2_ERC721_BRIDGE: Address = address!("0x4200000000000000000000000000000000000014");

    /// The Optimism mintable ERC721 proxy address.
    pub const OP_MINTABLE_ERC721_FACTORY: Address =
        address!("0x4200000000000000000000000000000000000017");

    /// The L2 proxy admin address.
    pub const PROXY_ADMIN: Address = address!("0x4200000000000000000000000000000000000018");

    /// The base fee vault address.
    pub const BASE_FEE_VAULT: Address = address!("0x4200000000000000000000000000000000000019");

    /// The L1 fee vault address.
    pub const L1_FEE_VAULT: Address = address!("0x420000000000000000000000000000000000001a");

    /// The schema registry proxy address.
    pub const SCHEMA_REGISTRY: Address = address!("0x4200000000000000000000000000000000000020");

    /// The EAS proxy address.
    pub const EAS: Address = address!("0x4200000000000000000000000000000000000021");

    /// Provides access to L1 beacon block roots (EIP-4788).
    pub const BEACON_BLOCK_ROOT: Address = address!("0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02");

    /// The Operator Fee Vault proxy address.
    pub const OPERATOR_FEE_VAULT: Address = address!("420000000000000000000000000000000000001B");

    /// The CrossL2Inbox proxy address.
    pub const CROSS_L2_INBOX: Address = address!("0x4200000000000000000000000000000000000022");

    /// The L2ToL2CrossDomainMessenger proxy address.
    pub const L2_TO_L2_XDM: Address = address!("0x4200000000000000000000000000000000000023");
}

/// Chain genesis information.
#[derive(Debug, Copy, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ChainGenesis {
    /// L1 genesis block
    pub l1: BlockNumHash,
    /// L2 genesis block
    pub l2: BlockNumHash,
    /// Timestamp of the L2 genesis block
    pub l2_time: u64,
    /// Optional System configuration
    pub system_config: Option<SystemConfig>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ChainGenesis {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let system_config = Option::<SystemConfig>::arbitrary(u)?;
        let l1_num_hash = BlockNumHash {
            number: u64::arbitrary(u)?,
            hash: alloy_primitives::B256::arbitrary(u)?,
        };
        let l2_num_hash = BlockNumHash {
            number: u64::arbitrary(u)?,
            hash: alloy_primitives::B256::arbitrary(u)?,
        };
        Ok(Self { l1: l1_num_hash, l2: l2_num_hash, l2_time: u.arbitrary()?, system_config })
    }
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;
    use alloy_primitives::{address, b256, uint};

    const fn ref_genesis() -> ChainGenesis {
        ChainGenesis {
            l1: BlockNumHash {
                hash: b256!("438335a20d98863a4c0c97999eb2481921ccd28553eac6f913af7c12aec04108"),
                number: 17422590,
            },
            l2: BlockNumHash {
                hash: b256!("dbf6a80fef073de06add9b0d14026d6e5a86c85f6d102c36d3d8e9cf89c2afd3"),
                number: 105235063,
            },
            l2_time: 1686068903,
            system_config: Some(SystemConfig {
                batcher_address: address!("6887246668a3b87F54DeB3b94Ba47a6f63F32985"),
                overhead: uint!(0xbc_U256),
                scalar: uint!(0xa6fe0_U256),
                gas_limit: 30000000,
                base_fee_scalar: None,
                blob_base_fee_scalar: None,
                eip1559_denominator: None,
                eip1559_elasticity: None,
                operator_fee_scalar: None,
                operator_fee_constant: None,
            }),
        }
    }

    #[test]
    fn test_genesis_serde() {
        let genesis_str = r#"{
            "l1": {
              "hash": "0x438335a20d98863a4c0c97999eb2481921ccd28553eac6f913af7c12aec04108",
              "number": 17422590
            },
            "l2": {
              "hash": "0xdbf6a80fef073de06add9b0d14026d6e5a86c85f6d102c36d3d8e9cf89c2afd3",
              "number": 105235063
            },
            "l2_time": 1686068903,
            "system_config": {
              "batcherAddress": "0x6887246668a3b87F54DeB3b94Ba47a6f63F32985",
              "overhead": "0x00000000000000000000000000000000000000000000000000000000000000bc",
              "scalar": "0x00000000000000000000000000000000000000000000000000000000000a6fe0",
              "gasLimit": 30000000
            }
          }"#;
        let genesis: ChainGenesis = serde_json::from_str(genesis_str).unwrap();
        assert_eq!(genesis, ref_genesis());
    }

    #[test]
    fn test_genesis_unknown_field_json() {
        let raw: &str = r#"{
            "l1": {
              "hash": "0x438335a20d98863a4c0c97999eb2481921ccd28553eac6f913af7c12aec04108",
              "number": 17422590
            },
            "l2": {
              "hash": "0xdbf6a80fef073de06add9b0d14026d6e5a86c85f6d102c36d3d8e9cf89c2afd3",
              "number": 105235063
            },
            "l2_time": 1686068903,
            "system_config": {
              "batcherAddress": "0x6887246668a3b87F54DeB3b94Ba47a6f63F32985",
              "overhead": "0x00000000000000000000000000000000000000000000000000000000000000bc",
              "scalar": "0x00000000000000000000000000000000000000000000000000000000000a6fe0",
              "gasLimit": 30000000
            },
            "unknown_field": "unknown"
        }"#;

        let err = serde_json::from_str::<ChainGenesis>(raw).unwrap_err();
        assert_eq!(err.classify(), serde_json::error::Category::Data);
    }
}
