//! Addresses of OP pre-deploys.
//!
//! This module contains the addresses of various predeploy contracts in the OP Stack.
//! See the complete set of predeploys at <https://specs.optimism.io/protocol/predeploys.html#predeploys>

use alloy_primitives::{Address, address};

/// Container for all predeploy contract addresses
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Predeploys;

impl Predeploys {
    /// List of all predeploys.
    pub const ALL: [Address; 18] = [
        Self::WETH9,
        Self::L2_CROSS_DOMAIN_MESSENGER,
        Self::L2_STANDARD_BRIDGE,
        Self::SEQUENCER_FEE_VAULT,
        Self::OP_MINTABLE_ERC20_FACTORY,
        Self::GAS_PRICE_ORACLE,
        Self::GOVERNANCE_TOKEN,
        Self::L2_ERC721_BRIDGE,
        Self::L1_BLOCK_INFO,
        Self::L2_TO_L1_MESSAGE_PASSER,
        Self::OP_MINTABLE_ERC721_FACTORY,
        Self::PROXY_ADMIN,
        Self::BASE_FEE_VAULT,
        Self::L1_FEE_VAULT,
        Self::SCHEMA_REGISTRY,
        Self::EAS,
        Self::OPERATOR_FEE_VAULT,
        Self::CROSS_L2_INBOX,
    ];

    /// The WETH9 predeploy address.
    pub const WETH9: Address = address!("0x4200000000000000000000000000000000000006");

    /// The L2 cross-domain messenger proxy address.
    pub const L2_CROSS_DOMAIN_MESSENGER: Address =
        address!("0x4200000000000000000000000000000000000007");

    /// The L2 standard bridge proxy address.
    pub const L2_STANDARD_BRIDGE: Address = address!("0x4200000000000000000000000000000000000010");

    /// The sequencer fee vault proxy address.
    pub const SEQUENCER_FEE_VAULT: Address = address!("0x4200000000000000000000000000000000000011");

    /// The Optimism mintable ERC20 factory proxy address.
    pub const OP_MINTABLE_ERC20_FACTORY: Address =
        address!("0x4200000000000000000000000000000000000012");

    /// The gas price oracle proxy address.
    pub const GAS_PRICE_ORACLE: Address = address!("0x420000000000000000000000000000000000000F");

    /// The governance token proxy address.
    pub const GOVERNANCE_TOKEN: Address = address!("0x4200000000000000000000000000000000000042");

    /// The L2 ERC721 bridge proxy address.
    pub const L2_ERC721_BRIDGE: Address = address!("0x4200000000000000000000000000000000000014");

    /// The L1 block information proxy address.
    pub const L1_BLOCK_INFO: Address = address!("0x4200000000000000000000000000000000000015");

    /// The L2 contract `L2ToL1MessagePasser`, stores commitments to withdrawal transactions.
    pub const L2_TO_L1_MESSAGE_PASSER: Address =
        address!("0x4200000000000000000000000000000000000016");

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

    /// The Operator Fee Vault proxy address.
    pub const OPERATOR_FEE_VAULT: Address = address!("420000000000000000000000000000000000001B");

    /// The CrossL2Inbox proxy address.
    pub const CROSS_L2_INBOX: Address = address!("0x4200000000000000000000000000000000000022");

    /// The L2ToL2CrossDomainMessenger proxy address.
    pub const L2_TO_L2_XDM: Address = address!("0x4200000000000000000000000000000000000023");
}
