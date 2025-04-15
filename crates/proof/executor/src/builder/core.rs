//! The [StatelessL2Builder] is a block builder that pulls state from a [TrieDB] during execution.

use crate::{ExecutorError, ExecutorResult, TrieDB, TrieDBError, TrieDBProvider};
use alloc::{string::ToString, vec::Vec};
use alloy_consensus::{Header, Sealed, transaction::Recovered};
use alloy_evm::{
    EvmFactory, FromRecoveredTx, FromTxWithEncoded,
    block::{BlockExecutionResult, BlockExecutor, BlockExecutorFactory},
};
use alloy_op_evm::{OpBlockExecutionCtx, OpBlockExecutorFactory, block::OpAlloyReceiptBuilder};
use alloy_primitives::{SignatureError, hex};
use kona_genesis::RollupConfig;
use kona_mpt::TrieHinter;
use op_alloy_consensus::{OpReceiptEnvelope, OpTxEnvelope};
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use op_revm::OpSpecId;
use revm::database::{State, states::bundle_state::BundleRetention};

/// The [`StatelessL2Builder`] is an OP Stack block builder that traverses a merkle patricia trie
/// via the [`TrieDB`] during execution.
#[derive(Debug)]
pub struct StatelessL2Builder<'a, P, H, Evm>
where
    P: TrieDBProvider,
    H: TrieHinter,
    Evm: EvmFactory,
{
    /// The [RollupConfig].
    pub(crate) config: &'a RollupConfig,
    /// The inner trie database.
    pub(crate) trie_db: TrieDB<P, H>,
    /// The executor factory, used to create new [`op_revm::OpEvm`] instances for block building
    /// routines.
    pub(crate) factory: OpBlockExecutorFactory<OpAlloyReceiptBuilder, RollupConfig, Evm>,
}

impl<'a, P, H, Evm> StatelessL2Builder<'a, P, H, Evm>
where
    P: TrieDBProvider,
    H: TrieHinter,
    Evm: EvmFactory<Spec = OpSpecId> + 'static,
    <Evm as EvmFactory>::Tx: FromTxWithEncoded<OpTxEnvelope> + FromRecoveredTx<OpTxEnvelope>,
{
    /// Creates a new [StatelessL2Builder] instance.
    pub fn new(
        config: &'a RollupConfig,
        evm_factory: Evm,
        provider: P,
        hinter: H,
        parent_header: Sealed<Header>,
    ) -> Self {
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        let factory = OpBlockExecutorFactory::new(
            OpAlloyReceiptBuilder::default(),
            config.clone(),
            evm_factory,
        );
        Self { config, trie_db, factory }
    }

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    pub fn build_block(
        &mut self,
        attrs: OpPayloadAttributes,
    ) -> ExecutorResult<BlockBuildingOutcome> {
        // Step 1. Set up the execution environment.
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

        // Attempt to send a payload witness hint to the host. This hint instructs the host to
        // populate its preimage store with the preimages required to statelessly execute
        // this payload. This feature is experimental, so if the hint fails, we continue
        // without it and fall back on on-demand preimage fetching for execution.
        self.trie_db
            .hinter
            .hint_execution_witness(parent_hash, &attrs)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?;

        info!(
            target: "block_builder",
            block_number = block_env.number,
            block_timestamp = block_env.timestamp,
            block_gas_limit = block_env.gas_limit,
            transactions = attrs.transactions.as_ref().map_or(0, |txs| txs.len()),
            "Beginning block building."
        );

        // Step 2. Create the executor, using the trie database.
        let mut state = State::builder()
            .with_database(&mut self.trie_db)
            .with_bundle_update()
            .without_state_clear()
            .build();
        let evm = self.factory.evm_factory().create_evm(&mut state, evm_env);
        let ctx = OpBlockExecutionCtx {
            parent_hash,
            parent_beacon_block_root: attrs.payload_attributes.parent_beacon_block_root,
            // This field is unused for individual block building jobs.
            extra_data: Default::default(),
        };
        let executor = self.factory.create_executor(evm, ctx);

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let transactions = attrs
            .recovered_transactions()
            .collect::<Result<Vec<Recovered<OpTxEnvelope>>, SignatureError>>()
            .map_err(ExecutorError::SignatureError)?;
        let ex_result = executor.execute_block(transactions.iter())?;

        info!(
            target: "block_builder",
            gas_used = ex_result.gas_used,
            gas_limit = block_env.gas_limit,
            "Finished block building. Beginning sealing job."
        );

        // Step 4. Merge state transitions and seal the block.
        state.merge_transitions(BundleRetention::Reverts);
        let bundle = state.take_bundle();
        let header = self.seal_block(&attrs, parent_hash, &block_env, &ex_result, bundle)?;

        info!(
            target: "block_builder",
            number = header.number,
            hash = hex::encode(header.seal()),
            state_root = hex::encode(header.state_root),
            transactions_root = hex::encode(header.transactions_root),
            receipts_root = hex::encode(header.receipts_root),
            "Sealed new block",
        );

        // Update the parent block hash in the state database, preparing for the next block.
        self.trie_db.set_parent_block_header(header.clone());
        Ok((header, ex_result).into())
    }
}

/// The outcome of a block building operation, returning the sealed block [`Header`] and the
/// [`BlockExecutionResult`].
#[derive(Debug, Clone)]
pub struct BlockBuildingOutcome {
    /// The block header.
    pub header: Sealed<Header>,
    /// The block execution result.
    pub execution_result: BlockExecutionResult<OpReceiptEnvelope>,
}

impl From<(Sealed<Header>, BlockExecutionResult<OpReceiptEnvelope>)> for BlockBuildingOutcome {
    fn from(
        (header, execution_result): (Sealed<Header>, BlockExecutionResult<OpReceiptEnvelope>),
    ) -> Self {
        Self { header, execution_result }
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::run_test_fixture;
    use rstest::rstest;
    use std::path::PathBuf;

    // To create new test fixtures, uncomment the following test and run it with parameters filled.
    // #[tokio::test(flavor = "multi_thread")]
    // async fn create_fixture() {
    //     let fixture_creator = crate::test_utils::ExecutorTestFixtureCreator::new(
    //         "<rpc_url>",
    //         <block_number>,
    //         PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata"),
    //     );
    //     fixture_creator.create_static_fixture().await;
    // }

    #[rstest]
    #[case::small_block(26207960)] // OP Sepolia
    #[case::small_block_2(26207961)] // OP Sepolia
    #[case::medium_block(26207962)] // OP Sepolia
    #[case::medium_block_2(26207963)] // OP Sepolia
    #[case::medium_block_3(26208927)] // OP Sepolia
    #[case::medium_block_4(26208858)] // OP Sepolia
    #[case::big_block(26208858)] // OP Sepolia
    #[case::big_block_2(26208384)] // OP Sepolia
    #[case::big_block_3(26211680)] // OP Sepolia
    #[tokio::test]
    async fn test_statelessly_execute_block(#[case] block_number: u64) {
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(format!("block-{block_number}.tar.gz"));

        run_test_fixture(fixture_dir).await;
    }
}
