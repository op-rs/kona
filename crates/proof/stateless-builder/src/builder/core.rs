//! The [StatelessL2Builder] is a block builder that pulls state from a [TrieDB] during execution.

use crate::{
    ExecutorError, ExecutorResult, TrieDB, TrieDBProvider, constants::SHA256_EMPTY,
    util::encode_holocene_eip_1559_params,
};
use alloc::vec::Vec;
use alloy_consensus::{EMPTY_OMMER_ROOT_HASH, Header, Sealed};
use alloy_eips::Encodable2718;
use alloy_evm::{
    EvmFactory,
    block::{BlockExecutionResult, BlockExecutor, BlockExecutorFactory},
};
use alloy_op_evm::{
    OpBlockExecutionCtx, OpBlockExecutorFactory, OpEvmFactory, block::OpAlloyReceiptBuilder,
};
use alloy_primitives::{B256, Sealable, U256, hex, logs_bloom};
use alloy_trie::EMPTY_ROOT_HASH;
use kona_genesis::RollupConfig;
use kona_mpt::{TrieHinter, ordered_trie_with_encoder};
use op_alloy_consensus::OpReceiptEnvelope;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use revm::{
    context::BlockEnv,
    context_interface::block::calc_excess_blob_gas,
    database::{BundleState, State, states::bundle_state::BundleRetention},
};

/// The [StatelessL2Builder] is a block builder that traverses a merkle patricia trie via the [TrieDB] during execution.
#[derive(Debug)]
pub struct StatelessL2Builder<'a, F, H>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    /// The [RollupConfig].
    pub(crate) config: &'a RollupConfig,
    /// The inner trie database.
    pub(crate) trie_db: TrieDB<F, H>,
    /// The executor factory, used to create new [op_revm::OpEvm] instances for block building routines.
    pub(crate) factory: OpBlockExecutorFactory<OpAlloyReceiptBuilder, RollupConfig, OpEvmFactory>,
}

impl<'a, F, H> StatelessL2Builder<'a, F, H>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    /// Creates a new [StatelessL2Builder] instance.
    pub fn new(
        config: &'a RollupConfig,
        provider: F,
        hinter: H,
        parent_header: Sealed<Header>,
    ) -> Self {
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        let factory = OpBlockExecutorFactory::new(
            OpAlloyReceiptBuilder::default(),
            config.clone(),
            OpEvmFactory::default(),
        );
        Self { config, trie_db, factory }
    }

    /// Builds a new block on top of the parent state, using the given [OpPayloadAttributes].
    ///
    /// TODO: Use `execute_block` instead of steps 2,3,4, once it exists in the [BlockExecutor] trait in a released
    /// version.
    pub fn build_block(
        &mut self,
        attrs: OpPayloadAttributes,
    ) -> ExecutorResult<(Sealed<Header>, BlockExecutionResult<OpReceiptEnvelope>)> {
        // Step 1. Set up the environment.
        let base_fee_params =
            Self::active_base_fee_params(self.config, self.trie_db.parent_block_header(), &attrs)?;
        let evm_env = self.evm_env(
            self.config.spec_id(attrs.payload_attributes.timestamp),
            self.trie_db.parent_block_header(),
            &attrs,
            &base_fee_params,
        )?;
        let block_env = evm_env.block_env().clone();
        let parent_hash = self.trie_db.parent_block_header().seal();

        info!(
            target: "block_builder",
            block_number = block_env.number,
            block_timestamp = block_env.timestamp,
            block_gas_limit = block_env.gas_limit,
            transactions = attrs.transactions.as_ref().map_or(0, |txs| txs.len()),
            "Beginning block building."
        );

        // Step 2. Create the executor, using the trie database.
        let mut state =
            State::builder().with_database(&mut self.trie_db).with_bundle_update().build();
        let evm = self.factory.evm_factory().create_evm(&mut state, evm_env);
        let ctx = OpBlockExecutionCtx {
            parent_hash,
            parent_beacon_block_root: attrs.payload_attributes.parent_beacon_block_root,
            extra_data: Default::default(),
        };
        let mut executor = self.factory.create_executor(evm, ctx);

        // Step 3. Apply pre-execution state changes.
        executor.apply_pre_execution_changes()?;

        // Step 4. Execute the transactions within the payload attributes.
        let transactions = attrs
            .transactions
            .clone()
            .map(|txs| {
                txs.iter().map(Self::decode_and_recover_tx).collect::<ExecutorResult<Vec<_>>>()
            })
            .ok_or(ExecutorError::MissingTransactions)??;
        for tx in transactions {
            // TODO: cumulative gas used checks.

            let gas_used = executor.execute_transaction(&tx)?;

            debug!(
                target: "block_builder",
                tx_hash = hex::encode(tx.seal().hash()),
                gas_used = gas_used,
                "Executed transaction",
            );
        }

        // Step 5. Apply post-execution state changes and finalize the block's transaction execution phase.
        let ex_result = executor.apply_post_execution_changes()?;

        info!(
            target: "block_builder",
            gas_used = ex_result.gas_used,
            gas_limit = block_env.gas_limit,
            "Finished block execution. Beginning sealing job."
        );

        // Step 6. Merge state transitions and seal the block.
        state.merge_transitions(BundleRetention::Reverts);
        let bundle = state.take_bundle();
        let header = self.seal_block(&attrs, parent_hash, &block_env, &ex_result, bundle)?;

        info!(
            target: "block_builder",
            hash = hex::encode(header.seal()),
            number = header.number,
            state_root = hex::encode(header.state_root),
            transactions_root = hex::encode(header.transactions_root),
            receipts_root = hex::encode(header.receipts_root),
            "Sealed new block",
        );

        // Update the parent block hash in the state database, preparing for the next block.
        self.trie_db.set_parent_block_header(header.clone());
        Ok((header, ex_result))
    }

    /// Seals the block executed from the given [OpPayloadAttributes] and [BlockEnv], returning the computed [Header].
    fn seal_block(
        &mut self,
        attrs: &OpPayloadAttributes,
        parent_hash: B256,
        block_env: &BlockEnv,
        ex_result: &BlockExecutionResult<OpReceiptEnvelope>,
        bundle: BundleState,
    ) -> ExecutorResult<Sealed<Header>> {
        // Compute the roots for the block header.
        let state_root = self.trie_db.state_root(&bundle)?;
        let transactions_root = ordered_trie_with_encoder(
            attrs.transactions.as_ref().expect("Transactions must be non-empty"),
            |tx, buf| buf.put_slice(tx.as_ref()),
        )
        .root();
        let receipts_root = Self::compute_receipts_root(
            &ex_result.receipts,
            self.config,
            attrs.payload_attributes.timestamp,
        );
        let withdrawals_root = if self.config.is_isthmus_active(attrs.payload_attributes.timestamp)
        {
            todo!("Message passer account root");
        } else if self.config.is_canyon_active(attrs.payload_attributes.timestamp) {
            Some(EMPTY_ROOT_HASH)
        } else {
            None
        };

        // Compute the logs bloom from the receipts generated during block execution.
        let logs_bloom = logs_bloom(ex_result.receipts.iter().flat_map(|r| r.logs()));

        // Compute Cancun fields, if active.
        let (blob_gas_used, excess_blob_gas) = self
            .config
            .is_ecotone_active(attrs.payload_attributes.timestamp)
            .then(|| {
                let parent_header = self.trie_db.parent_block_header();
                let excess_blob_gas = if self.config.is_ecotone_active(parent_header.timestamp) {
                    let parent_excess_blob_gas = parent_header.excess_blob_gas.unwrap_or_default();
                    let parent_blob_gas_used = parent_header.blob_gas_used.unwrap_or_default();

                    calc_excess_blob_gas(parent_excess_blob_gas, parent_blob_gas_used, 0)
                } else {
                    // For the first post-fork block, both blob gas fields are evaluated to 0.
                    calc_excess_blob_gas(0, 0, 0)
                };

                (Some(0), Some(excess_blob_gas as u128))
            })
            .unwrap_or_default();

        // At holocene activation, the base fee parameters from the payload are placed
        // into the Header's `extra_data` field.
        //
        // If the payload's `eip_1559_params` are equal to `0`, then the header's `extraData`
        // field is set to the encoded canyon base fee parameters.
        let encoded_base_fee_params = self
            .config
            .is_holocene_active(attrs.payload_attributes.timestamp)
            .then(|| encode_holocene_eip_1559_params(self.config, attrs))
            .transpose()?
            .unwrap_or_default();

        // The requests hash on the OP Stack, if Isthmus is active, is always the empty SHA256 hash.
        let requests_hash = self
            .config
            .is_isthmus_active(attrs.payload_attributes.timestamp)
            .then_some(SHA256_EMPTY);

        // Construct the new header.
        let header = Header {
            parent_hash,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: attrs.payload_attributes.suggested_fee_recipient,
            state_root,
            transactions_root,
            receipts_root,
            withdrawals_root,
            requests_hash,
            logs_bloom,
            difficulty: U256::ZERO,
            number: block_env.number,
            gas_limit: attrs.gas_limit.ok_or(ExecutorError::MissingGasLimit)?,
            gas_used: ex_result.gas_used,
            timestamp: attrs.payload_attributes.timestamp,
            mix_hash: attrs.payload_attributes.prev_randao,
            nonce: Default::default(),
            base_fee_per_gas: Some(block_env.basefee),
            blob_gas_used,
            excess_blob_gas: excess_blob_gas.and_then(|x| x.try_into().ok()),
            parent_beacon_block_root: attrs.payload_attributes.parent_beacon_block_root,
            extra_data: encoded_base_fee_params,
        }
        .seal_slow();

        Ok(header)
    }

    /// Computes the receipts root from the given set of receipts.
    fn compute_receipts_root(
        receipts: &[OpReceiptEnvelope],
        config: &RollupConfig,
        timestamp: u64,
    ) -> B256 {
        // There is a minor bug in op-geth and op-erigon where in the Regolith hardfork,
        // the receipt root calculation does not inclide the deposit nonce in the
        // receipt encoding. In the Regolith hardfork, we must strip the deposit nonce
        // from the receipt encoding to match the receipt root calculation.
        if config.is_regolith_active(timestamp) && !config.is_canyon_active(timestamp) {
            let receipts = receipts
                .iter()
                .cloned()
                .map(|receipt| match receipt {
                    OpReceiptEnvelope::Deposit(mut deposit_receipt) => {
                        deposit_receipt.receipt.deposit_nonce = None;
                        OpReceiptEnvelope::Deposit(deposit_receipt)
                    }
                    _ => receipt,
                })
                .collect::<Vec<_>>();

            ordered_trie_with_encoder(receipts.as_ref(), |receipt, mut buf| {
                receipt.encode_2718(&mut buf)
            })
            .root()
        } else {
            ordered_trie_with_encoder(receipts, |receipt, mut buf| receipt.encode_2718(&mut buf))
                .root()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::run_test_fixture;
    use rstest::rstest;
    use std::path::PathBuf;

    // To create new test fixtures, uncomment the following test and run it with parameters filled.
    #[tokio::test(flavor = "multi_thread")]
    async fn create_fixture() {
        let fixture_creator = crate::test_utils::ExecutorTestFixtureCreator::new(
            "http://stf.clab.by:8548",
            25946052,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata"),
        );
        fixture_creator.create_static_fixture().await;
    }

    #[rstest]
    #[case::small_block(10311000)] // Unichain Mainnet
    #[case::small_block_2(10211000)] // Unichain Mainnet
    #[case::small_block_3(10215000)] // Unichain Mainnet
    #[case::medium_block_1(132795025)] // OP Mainnet
    #[case::medium_block_2(132796000)] // OP Mainnet
    #[case::medium_block_3(132797000)] // OP Mainnet
    #[case::medium_block_4(132798000)] // OP Mainnet
    #[case::medium_block_5(132799000)] // OP Mainnet
    #[tokio::test]
    async fn test_statelessly_execute_block(#[case] block_number: u64) {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(format!("block-{block_number}.tar.gz"));

        run_test_fixture(fixture_dir).await;
    }
}
