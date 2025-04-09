//! Contains a utility method to check if attributes match a block.

use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::{Block, Withdrawals};
use kona_genesis::RollupConfig;
use kona_rpc::OpAttributesWithParent;
use op_alloy_rpc_types::Transaction;

/// Represents whether the attributes match the block or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributesMatch {
    /// The attributes match the block.
    Match,
    /// The attributes do not match the block.
    Mismatch(AttributesMismatch),
}

impl AttributesMatch {
    /// Returns true if the attributes match the block.
    pub const fn is_match(&self) -> bool {
        matches!(self, Self::Match)
    }

    /// Returns true if the attributes do not match the block.
    pub const fn is_mismatch(&self) -> bool {
        matches!(self, Self::Mismatch(_))
    }

    /// Checks that withdrawals for a block and attributes match.
    pub fn check_withdrawals(
        config: &RollupConfig,
        attributes: &OpAttributesWithParent,
        block: &Block<Transaction>,
    ) -> Self {
        let attr_withdrawals = attributes.attributes.payload_attributes.withdrawals.as_ref();
        let attr_withdrawals = attr_withdrawals.map(|w| Withdrawals::new(w.to_vec()));
        let block_withdrawals = block.withdrawals.as_ref();

        if config.is_canyon_active(block.header.timestamp) {
            // In canyon, the withdrawals list should be some and empty
            if attr_withdrawals.is_none_or(|w| !w.is_empty()) {
                return Self::Mismatch(AttributesMismatch::CanyonWithdrawalsNotEmpty);
            }
            if block_withdrawals.is_none_or(|w| !w.is_empty()) {
                return Self::Mismatch(AttributesMismatch::CanyonWithdrawalsNotEmpty);
            }
            if !config.is_isthmus_active(block.header.timestamp) {
                // In canyon, the withdrawals root should be set to the empty value
                let empty_hash = alloy_consensus::EMPTY_ROOT_HASH;
                if block.header.inner.withdrawals_root != Some(empty_hash) {
                    return Self::Mismatch(AttributesMismatch::CanyonNotEmptyHash);
                }
            }
        } else {
            // In bedrock, the withdrawals list should be None
            if attr_withdrawals.is_some() {
                return Self::Mismatch(AttributesMismatch::BedrockWithdrawals);
            }
        }

        if config.is_isthmus_active(block.header.timestamp) {
            // In isthmus, the withdrawals root must be set
            if block.header.inner.withdrawals_root.is_none() {
                return Self::Mismatch(AttributesMismatch::IsthmusMissingWithdrawalsRoot);
            }
        }

        Self::Match
    }

    /// Checks if the specified [`OpAttributesWithParent`] matches the specified [`Block`].
    /// Returns [`AttributesMatch::Match`] if they match, otherwise returns
    /// [`AttributesMatch::Mismatch`].
    pub fn check(
        config: &RollupConfig,
        attributes: &OpAttributesWithParent,
        block: &Block<Transaction>,
    ) -> Self {
        if attributes.parent.block_info.hash != block.header.inner.parent_hash {
            return AttributesMismatch::ParentHash(
                attributes.parent.block_info.hash,
                block.header.inner.parent_hash,
            )
            .into();
        }

        if attributes.attributes.payload_attributes.timestamp != block.header.inner.timestamp {
            return AttributesMismatch::Timestamp(
                attributes.attributes.payload_attributes.timestamp,
                block.header.inner.timestamp,
            )
            .into();
        }

        let mix_hash = block.header.inner.mix_hash;
        if attributes.attributes.payload_attributes.prev_randao != mix_hash {
            return AttributesMismatch::PrevRandao(
                attributes.attributes.payload_attributes.prev_randao,
                mix_hash,
            )
            .into();
        }

        // TODO: check transactions

        let Some(gas_limit) = attributes.attributes.gas_limit else {
            return AttributesMismatch::MissingAttributesGasLimit.into();
        };

        if gas_limit != block.header.inner.gas_limit {
            return AttributesMismatch::GasLimit(gas_limit, block.header.inner.gas_limit).into();
        }

        if let Self::Mismatch(m) = Self::check_withdrawals(config, attributes, block) {
            return m.into();
        }

        if attributes.attributes.payload_attributes.parent_beacon_block_root !=
            block.header.inner.parent_beacon_block_root
        {
            return AttributesMismatch::ParentBeaconBlockRoot(
                attributes.attributes.payload_attributes.parent_beacon_block_root,
                block.header.inner.parent_beacon_block_root,
            )
            .into();
        }

        if attributes.attributes.payload_attributes.suggested_fee_recipient !=
            block.header.inner.beneficiary
        {
            return AttributesMismatch::FeeRecipient(
                attributes.attributes.payload_attributes.suggested_fee_recipient,
                block.header.inner.beneficiary,
            )
            .into();
        }

        // TODO: Check EIP-1559 parameters

        Self::Match
    }
}

/// An enum over the type of mismatch between [`OpAttributesWithParent`]
/// and a [`Block`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributesMismatch {
    /// The parent hash of the block does not match the parent hash of the attributes.
    ParentHash(B256, B256),
    /// The timestamp of the block does not match the timestamp of the attributes.
    Timestamp(u64, u64),
    /// The prev randao of the block does not match the prev randao of the attributes.
    PrevRandao(B256, B256),
    /// Transactions mismatch.
    Transactions(u64, u64),
    /// The gas limit of the block does not match the gas limit of the attributes.
    GasLimit(u64, u64),
    /// The gas limit for the [`OpAttributesWithParent`] is missing.
    MissingAttributesGasLimit,
    /// The fee recipient of the block does not match the fee recipient of the attributes.
    FeeRecipient(Address, Address),
    /// A mismatch in the parent beacon block root.
    ParentBeaconBlockRoot(Option<B256>, Option<B256>),
    /// After the canyon hardfork, withdrawals cannot be empty.
    CanyonWithdrawalsNotEmpty,
    /// After the canyon hardfork, the withdrawals root must be the empty hash.
    CanyonNotEmptyHash,
    /// In the bedrock hardfork, the attributes must has empty withdrawals.
    BedrockWithdrawals,
    /// In the isthmus hardfork, the withdrawals root must be set.
    IsthmusMissingWithdrawalsRoot,
}

impl From<AttributesMismatch> for AttributesMatch {
    fn from(mismatch: AttributesMismatch) -> Self {
        Self::Mismatch(mismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, b256};
    use kona_protocol::L2BlockInfo;
    use kona_registry::ROLLUP_CONFIGS;
    use op_alloy_rpc_types_engine::OpPayloadAttributes;

    fn default_attributes() -> OpAttributesWithParent {
        OpAttributesWithParent {
            attributes: OpPayloadAttributes::default(),
            parent: L2BlockInfo::default(),
            is_last_in_span: true,
        }
    }

    fn default_rollup_config() -> &'static RollupConfig {
        let opm = 10;
        ROLLUP_CONFIGS.get(&opm).expect("default rollup config should exist")
    }

    #[test]
    fn test_attributes_match_parent_hash_mismatch() {
        let cfg = default_rollup_config();
        let attributes = default_attributes();
        let mut block = Block::<Transaction>::default();
        block.header.inner.parent_hash =
            b256!("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef");
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::ParentHash(
            attributes.parent.block_info.hash,
            block.header.inner.parent_hash,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_check_timestamp() {
        let cfg = default_rollup_config();
        let attributes = default_attributes();
        let mut block = Block::<Transaction>::default();
        block.header.inner.timestamp = 1234567890;
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::Timestamp(
            attributes.attributes.payload_attributes.timestamp,
            block.header.inner.timestamp,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_check_prev_randao() {
        let cfg = default_rollup_config();
        let attributes = default_attributes();
        let mut block = Block::<Transaction>::default();
        block.header.inner.mix_hash =
            b256!("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef");
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::PrevRandao(
            attributes.attributes.payload_attributes.prev_randao,
            block.header.inner.mix_hash,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_missing_gas_limit() {
        let cfg = default_rollup_config();
        let attributes = default_attributes();
        let mut block = Block::<Transaction>::default();
        block.header.inner.gas_limit = 123456;
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::MissingAttributesGasLimit.into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_check_gas_limit() {
        let cfg = default_rollup_config();
        let mut attributes = default_attributes();
        attributes.attributes.gas_limit = Some(123457);
        let mut block = Block::<Transaction>::default();
        block.header.inner.gas_limit = 123456;
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::GasLimit(
            attributes.attributes.gas_limit.unwrap_or_default(),
            block.header.inner.gas_limit,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_check_parent_beacon_block_root() {
        let cfg = default_rollup_config();
        let mut attributes = default_attributes();
        attributes.attributes.gas_limit = Some(0);
        attributes.attributes.payload_attributes.parent_beacon_block_root =
            Some(b256!("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"));
        let block = Block::<Transaction>::default();
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::ParentBeaconBlockRoot(
            attributes.attributes.payload_attributes.parent_beacon_block_root,
            block.header.inner.parent_beacon_block_root,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match_check_fee_recipient() {
        let cfg = default_rollup_config();
        let mut attributes = default_attributes();
        attributes.attributes.gas_limit = Some(0);
        let mut block = Block::<Transaction>::default();
        block.header.inner.beneficiary = address!("1234567890abcdef1234567890abcdef12345678");
        let check = AttributesMatch::check(cfg, &attributes, &block);
        let expected: AttributesMatch = AttributesMismatch::FeeRecipient(
            attributes.attributes.payload_attributes.suggested_fee_recipient,
            block.header.inner.beneficiary,
        )
        .into();
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_match() {
        let cfg = default_rollup_config();
        let mut attributes = default_attributes();
        attributes.attributes.gas_limit = Some(0);
        let block = Block::<Transaction>::default();
        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, AttributesMatch::Match);
        assert!(check.is_match());
    }
}
