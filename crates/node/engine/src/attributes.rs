//! Contains a utility method to check if attributes match a block.

use alloy_network::TransactionResponse;
use alloy_primitives::{Address, B256, Bytes};
use alloy_rpc_types_eth::{Block, BlockTransactions, Withdrawals};
use kona_genesis::RollupConfig;
use kona_rpc::OpAttributesWithParent;
use op_alloy_consensus::OpTxEnvelope;
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

    /// Checks the attributes and block transaction list for consolidation.
    /// We start by checking that there are the same number of transactions in both the attribute
    /// payload and the block. Then we compare their contents
    fn check_transactions(attributes_txs: &[Bytes], block: &Block<Transaction>) -> Self {
        // Before checking the number of transactions, we have to make sure that the block
        // has the right transactions format. We need to have access to the
        // full transactions to be able to compare their contents.
        let block_txs = match block.transactions {
            BlockTransactions::Hashes(_) | BlockTransactions::Full(_)
                if attributes_txs.is_empty() && block.transactions.is_empty() =>
            {
                // We early return when both attributes and blocks are empty. This is for ergonomics
                // because the default [`BlockTransactions`] format is
                // [`BlockTransactions::Hash`], which may cause
                // the [`BlockTransactions`] format check to fail right below. We may want to be a
                // bit more flexible and not reject the hash format if both the
                // attributes and the block are empty.
                return Self::Match;
            }
            BlockTransactions::Uncle => {
                // This can never be uncle transactions
                error!(
                    "Invalid format for the block transactions. The `Uncle` transaction format is not relevant in that context and should not get used here. This is a bug"
                );

                return AttributesMismatch::MalformedBlockTransactions.into();
            }
            BlockTransactions::Hashes(_) => {
                // We can't have hash transactions with non empty blocks
                error!(
                    "Invalid format for the block transactions. The `Hash` transaction format is not relevant in that context and should not get used here. This is a bug."
                );

                return AttributesMismatch::MalformedBlockTransactions.into();
            }
            BlockTransactions::Full(ref block_txs) => block_txs,
        };

        let attributes_txs_len = attributes_txs.len();
        let block_txs_len = block_txs.len();

        if attributes_txs_len != block_txs_len {
            return AttributesMismatch::TransactionLen(attributes_txs_len, block_txs_len).into();
        }

        // Then we need to check that the content of the encoded transactions match
        // Note that it is safe to zip both iterators because we checked their length
        // beforehand.
        for (attr_tx_bytes, block_tx) in attributes_txs.iter().zip(block_txs) {
            // Let's try to deserialize the attributes transaction
            let Ok(attr_tx) = serde_json::from_slice::<OpTxEnvelope>(attr_tx_bytes) else {
                error!(
                    "Impossible to deserialize transaction from attributes. If we have stored these attributes it means the transactions where well formatted. This is a bug"
                );

                return AttributesMismatch::MalformedAttributesTransaction.into();
            };

            if &attr_tx != block_tx.inner.inner.inner() {
                // Transaction mismatch!
                return AttributesMismatch::TransactionContent(attr_tx.tx_hash(), block_tx.tx_hash())
                    .into()
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

        // Let's extract the list of attribute transactions
        let default_vec = vec![];
        let attributes_txs =
            attributes.attributes.transactions.as_ref().map_or_else(|| &default_vec, |attrs| attrs);

        // Check transactions
        if let mismatch @ Self::Mismatch(_) = Self::check_transactions(attributes_txs, block) {
            return mismatch
        }

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
    /// The block contains malformed transactions. This is a bug - the transaction format
    /// should be checked before the consolidation step.
    MalformedBlockTransactions,
    /// There is a malformed transaction inside the attributes. This is a bug - the transaction
    /// format should be checked before the consolidation step.
    MalformedAttributesTransaction,
    /// A mismatch in the number of transactions contained in the attributes and the block.
    TransactionLen(usize, usize),
    /// A mismatch in the content of some transactions contained in the attributes and the block.
    TransactionContent(B256, B256),
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
    use alloy_primitives::{Bytes, address, b256};
    use alloy_rpc_types_eth::BlockTransactions;
    use arbitrary::{Arbitrary, Unstructured};
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

    fn generate_txs(num_txs: usize) -> Vec<Transaction> {
        // Simulate some random data
        let mut data = vec![0; 1024];
        let mut rng = rand::rng();

        (0..num_txs)
            .map(|_| {
                rand::Rng::fill(&mut rng, &mut data[..]);

                // Create unstructured data with the random bytes
                let u = Unstructured::new(&data);

                // Generate a random instance of MyStruct
                Transaction::arbitrary_take_rest(u).expect("Impossible to generate arbitrary tx")
            })
            .collect()
    }

    fn test_transactions_match_helper() -> (OpAttributesWithParent, Block<Transaction>) {
        const NUM_TXS: usize = 10;

        let transactions = generate_txs(NUM_TXS);
        let mut attributes = default_attributes();
        attributes.attributes.gas_limit = Some(0);
        attributes.attributes.transactions = Some(
            transactions
                .iter()
                .map(|tx| {
                    Bytes::from_iter(
                        serde_json::to_vec(tx.inner.inner.inner())
                            .expect("Impossible to serialize")
                            .iter(),
                    )
                })
                .collect::<Vec<_>>(),
        );

        let block = Block::<Transaction> {
            transactions: BlockTransactions::Full(transactions),
            ..Default::default()
        };

        (attributes, block)
    }

    #[test]
    fn test_attributes_match_check_transactions() {
        let cfg = default_rollup_config();
        let (attributes, block) = test_transactions_match_helper();
        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, AttributesMatch::Match);
    }

    #[test]
    fn test_attributes_mismatch_check_transactions_len() {
        let cfg = default_rollup_config();
        let (mut attributes, block) = test_transactions_match_helper();
        attributes.attributes = OpPayloadAttributes {
            transactions: attributes.attributes.transactions.map(|mut txs| {
                txs.pop();
                txs
            }),
            ..attributes.attributes
        };

        let block_txs_len = block.transactions.len();

        let expected: AttributesMatch =
            AttributesMismatch::TransactionLen(block_txs_len - 1, block_txs_len).into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    #[test]
    fn test_attributes_mismatch_check_transaction_content() {
        let cfg = default_rollup_config();
        let (attributes, mut block) = test_transactions_match_helper();
        let BlockTransactions::Full(block_txs) = &mut block.transactions else {
            unreachable!("The helper should build a full list of transactions")
        };

        let first_tx = block_txs.last().unwrap().clone();
        let first_tx_hash = first_tx.tx_hash();

        // We set the last tx to be the same as the first transaction.
        // Since the transactions are generated randomly and there are more than one transaction,
        // there is a very high likelihood that any pair of transactions is distinct.
        let last_tx = block_txs.first_mut().unwrap();
        let last_tx_hash = last_tx.tx_hash();
        *last_tx = first_tx;

        let expected: AttributesMatch =
            AttributesMismatch::TransactionContent(last_tx_hash, first_tx_hash).into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    /// Checks the edge case where the the attributes array is empty.
    #[test]
    fn test_attributes_mismatch_empty_tx_attributes() {
        let cfg = default_rollup_config();
        let (mut attributes, block) = test_transactions_match_helper();
        attributes.attributes = OpPayloadAttributes { transactions: None, ..attributes.attributes };

        let block_txs_len = block.transactions.len();

        let expected: AttributesMatch = AttributesMismatch::TransactionLen(0, block_txs_len).into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    /// Checks the edge case where the the transactions contained in the block have the wrong
    /// format.
    #[test]
    fn test_block_transactions_wrong_format() {
        let cfg = default_rollup_config();
        let (attributes, mut block) = test_transactions_match_helper();
        block.transactions = BlockTransactions::Uncle;

        let expected: AttributesMatch = AttributesMismatch::MalformedBlockTransactions.into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    /// Checks the edge case where the the transactions contained in the attributes have the wrong
    /// format.
    #[test]
    fn test_attributes_transactions_wrong_format() {
        let cfg = default_rollup_config();
        let (mut attributes, block) = test_transactions_match_helper();
        let txs = attributes.attributes.transactions.as_mut().unwrap();
        let first_tx_bytes = txs.first_mut().unwrap();
        *first_tx_bytes = Bytes::copy_from_slice(&[0, 1, 2]);

        let expected: AttributesMatch = AttributesMismatch::MalformedAttributesTransaction.into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, expected);
        assert!(check.is_mismatch());
    }

    // Test that the check pass if the transactions obtained from the attributes have the format
    // `Some(vec![])`, ie an empty vector inside a `Some` option.
    #[test]
    fn test_attributes_and_block_transactions_empty() {
        let cfg = default_rollup_config();
        let (mut attributes, mut block) = test_transactions_match_helper();

        attributes.attributes =
            OpPayloadAttributes { transactions: Some(vec![]), ..attributes.attributes };

        block.transactions = BlockTransactions::Full(vec![]);

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, AttributesMatch::Match);

        // Edge case: if the block transactions and the payload attributes are empty, we can also
        // use the hash format (this is the default value of `BlockTransactions`).
        attributes.attributes = OpPayloadAttributes { transactions: None, ..attributes.attributes };
        block.transactions = BlockTransactions::Hashes(vec![]);

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, AttributesMatch::Match);
    }

    // Edge case: if the payload attributes has the format `Some(vec![])`, we can still
    // use the hash format.
    #[test]
    fn test_attributes_and_block_transactions_empty_hash_format() {
        let cfg = default_rollup_config();
        let (mut attributes, mut block) = test_transactions_match_helper();

        attributes.attributes =
            OpPayloadAttributes { transactions: Some(vec![]), ..attributes.attributes };

        block.transactions = BlockTransactions::Hashes(vec![]);

        let check = AttributesMatch::check(cfg, &attributes, &block);
        assert_eq!(check, AttributesMatch::Match);
    }

    // Test that the check fails if the block format is incorrect and the attributes are empty
    #[test]
    fn test_attributes_empty_and_block_uncle() {
        let cfg = default_rollup_config();
        let (mut attributes, mut block) = test_transactions_match_helper();

        attributes.attributes =
            OpPayloadAttributes { transactions: Some(vec![]), ..attributes.attributes };

        block.transactions = BlockTransactions::Uncle;

        let expected: AttributesMatch = AttributesMismatch::MalformedBlockTransactions.into();

        let check = AttributesMatch::check(cfg, &attributes, &block);
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
