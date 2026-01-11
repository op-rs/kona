//! Stateless OP Stack L2 block builder implementation.
//!
//! The [StatelessL2Builder] provides a complete block building and execution engine
//! for OP Stack L2 chains that operates in a stateless manner, pulling required state
//! data from a [TrieDB] during execution rather than maintaining full state.

use crate::{ExecutorError, ExecutorResult, TrieDB, TrieDBError, TrieDBProvider};
use alloc::{string::ToString, vec::Vec};
use alloy_consensus::{Header, Sealed, crypto::RecoveryError};
use alloy_evm::{
    EvmFactory, FromRecoveredTx, FromTxWithEncoded,
    block::{BlockExecutionResult, BlockExecutor, BlockExecutorFactory},
};
use alloy_op_evm::{
    OpBlockExecutionCtx, OpBlockExecutorFactory,
    block::{OpAlloyReceiptBuilder, OpTxEnv},
};
use core::fmt::Debug;
use kona_genesis::RollupConfig;
use kona_mpt::TrieHinter;
use op_alloy_consensus::{OpReceiptEnvelope, OpTxEnvelope};
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use op_revm::OpSpecId;
use revm::{
    Inspector,
    context::BlockEnv,
    database::{State, states::bundle_state::BundleRetention},
    inspector::NoOpInspector,
};

/// A factory trait for creating EVM inspectors.
///
/// This trait allows for flexible inspector creation strategies while ensuring
/// that the inspector's lifetime is scoped to the block execution.
///
/// # Important: Inspector Implementation Requirements
///
/// The inspector type produced by this factory **must** implement `Inspector<Ctx>`
/// for **any** context type `Ctx`. This is required because the EVM context type
/// includes references to the state database, and the borrow checker needs to be
/// able to prove that the inspector doesn't capture these references.
///
/// The simplest way to achieve this is to implement `Inspector` generically:
///
/// ```rust,ignore
/// impl<Ctx> Inspector<Ctx> for MyInspector {
///     // ... method implementations that don't depend on Ctx's lifetime
/// }
/// ```
///
/// Inspectors that try to capture references from the context or store state
/// with lifetime dependencies will cause borrow checker errors.
///
/// # Example
///
/// ```rust,ignore
/// use revm::{Inspector, inspector::NoOpInspector};
///
/// // A simple inspector that logs call depth
/// #[derive(Debug, Clone)]
/// struct CallDepthLogger {
///     max_depth: usize,
/// }
///
/// // Implement Inspector for any context type
/// impl<Ctx> Inspector<Ctx> for CallDepthLogger {
///     // Implementation here
/// }
///
/// struct CallDepthLoggerFactory;
///
/// impl InspectorFactory for CallDepthLoggerFactory {
///     type Inspector = CallDepthLogger;
///
///     fn create(&self) -> Self::Inspector {
///         CallDepthLogger { max_depth: 0 }
///     }
/// }
/// ```
pub trait InspectorFactory {
    /// The inspector type produced by this factory.
    ///
    /// This type must implement `Inspector<Ctx>` for any context type `Ctx`.
    type Inspector;

    /// Creates a new inspector instance.
    ///
    /// This method is called once per block execution to create a fresh inspector.
    fn create(&self) -> Self::Inspector;
}

/// Implementation of [`InspectorFactory`] for the unit type `()`.
///
/// This is used as the default type parameter for [`StatelessL2Builder`] and
/// satisfies the `IF: InspectorFactory` bound when `None::<()>` is passed.
/// The produced inspector is [`NoOpInspector`].
impl InspectorFactory for () {
    type Inspector = NoOpInspector;

    fn create(&self) -> Self::Inspector {
        NoOpInspector
    }
}

/// Stateless OP Stack L2 block builder that derives state from trie proofs during execution.
///
/// The [`StatelessL2Builder`] is a specialized block execution engine designed for fault proof
/// systems and stateless verification. Instead of maintaining full L2 state, it dynamically
/// retrieves required state data from a [`TrieDB`] backed by Merkle proofs and witnesses.
///
/// # Architecture
///
/// The builder operates in a stateless manner by:
/// 1. **Trie Database**: Uses [`TrieDB`] to access state via Merkle proofs
/// 2. **EVM Factory**: Creates execution environments with proof-backed state
/// 3. **Block Executor**: Executes transactions using witness-provided state
/// 4. **Receipt Generation**: Produces execution receipts and state commitments
///
/// # Stateless Execution Model
///
/// Traditional execution engines maintain full state databases, but the stateless model:
/// - Receives state witnesses containing only required data
/// - Verifies state access against Merkle proofs
/// - Executes transactions without persistent state storage
/// - Produces verifiable execution results and state commitments
///
/// # Use Cases
///
/// ## Fault Proof Systems
/// - Enables dispute resolution without full state replication
/// - Provides verifiable execution results for challenge games
/// - Supports optimistic rollup fraud proof generation
///
/// ## Stateless Verification
/// - Allows third parties to verify L2 blocks without full state
/// - Enables light clients to validate L2 execution
/// - Supports decentralized verification networks
///
/// # Performance Characteristics
///
/// - **Memory**: Lower memory usage than stateful execution (no full state)
/// - **I/O**: Higher I/O for proof verification and witness access
/// - **CPU**: Additional overhead for cryptographic proof verification
/// - **Determinism**: Guaranteed deterministic execution results
///
/// # Type Parameters
///
/// * `P` - Trie database provider implementing [`TrieDBProvider`]
/// * `H` - Trie hinter implementing [`TrieHinter`] for state access optimization
/// * `Evm` - EVM factory implementing [`EvmFactory`] for execution environment creation
/// * `IF` - Optional inspector factory implementing [`InspectorFactory`] for creating EVM
///   inspectors
#[derive(Debug)]
pub struct StatelessL2Builder<'a, P, H, Evm, IF>
where
    P: TrieDBProvider,
    H: TrieHinter,
    Evm: EvmFactory,
    IF: InspectorFactory,
{
    /// The rollup configuration containing chain parameters and activation heights.
    ///
    /// Provides access to network-specific parameters including gas limits,
    /// hard fork activation heights, and system addresses needed for proper
    /// L2 block execution and validation.
    pub(crate) config: &'a RollupConfig,
    /// The trie database providing stateless access to L2 state via Merkle proofs.
    ///
    /// The [`TrieDB`] serves as the primary interface for state access during
    /// execution, resolving account and storage queries using witness data
    /// and cryptographic proofs rather than a traditional state database.
    pub(crate) trie_db: TrieDB<P, H>,
    /// The block executor factory for creating OP Stack execution environments.
    ///
    /// This factory creates specialized OP Stack execution environments that
    /// understand OP-specific transaction types, system calls, and state
    /// management required for proper L2 block execution.
    pub(crate) factory: OpBlockExecutorFactory<OpAlloyReceiptBuilder, RollupConfig, Evm>,
    /// The optional inspector factory for creating EVM inspectors.
    ///
    /// When `Some`, uses [`EvmFactory::create_evm_with_inspector`] with the factory-created
    /// inspector to trace or monitor EVM execution.
    /// When `None`, uses [`EvmFactory::create_evm`] without any custom inspector.
    pub(crate) inspector_factory: Option<IF>,
}

impl<'a, P, H, Evm, IF> StatelessL2Builder<'a, P, H, Evm, IF>
where
    P: TrieDBProvider + Debug,
    H: TrieHinter + Debug,
    Evm: EvmFactory<Spec = OpSpecId, BlockEnv = BlockEnv> + 'static,
    <Evm as EvmFactory>::Tx:
        FromTxWithEncoded<OpTxEnvelope> + FromRecoveredTx<OpTxEnvelope> + OpTxEnv,
    IF: InspectorFactory,
{
    /// Creates a new stateless L2 block builder instance.
    ///
    /// Initializes the builder with the necessary components for stateless block execution
    /// including the trie database, execution factory, and rollup configuration.
    ///
    /// # Arguments
    /// * `config` - Rollup configuration with chain parameters and activation heights
    /// * `evm_factory` - EVM factory for creating execution environments
    /// * `provider` - Trie database provider for state access
    /// * `hinter` - Trie hinter for optimizing state access patterns
    /// * `parent_header` - Sealed header of the parent block to build upon
    /// * `inspector_factory` - Optional inspector factory for creating EVM inspectors. Pass `None`
    ///   to use the default no-op inspector.
    ///
    /// # Returns
    /// A new [`StatelessL2Builder`] ready for block building operations
    ///
    /// # Usage
    /// ```rust,ignore
    /// // Without custom inspector (recommended for most use cases)
    /// let builder = StatelessL2Builder::new(
    ///     &rollup_config,
    ///     evm_factory,
    ///     trie_provider,
    ///     trie_hinter,
    ///     parent_header,
    ///     None,
    /// );
    ///
    /// // With custom inspector factory
    /// let builder = StatelessL2Builder::new(
    ///     &rollup_config,
    ///     evm_factory,
    ///     trie_provider,
    ///     trie_hinter,
    ///     parent_header,
    ///     Some(MyInspectorFactory),
    /// );
    /// ```
    pub fn new(
        config: &'a RollupConfig,
        evm_factory: Evm,
        provider: P,
        hinter: H,
        parent_header: Sealed<Header>,
        inspector_factory: Option<IF>,
    ) -> Self {
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        let factory = OpBlockExecutorFactory::new(
            OpAlloyReceiptBuilder::default(),
            config.clone(),
            evm_factory,
        );
        Self { config, trie_db, factory, inspector_factory }
    }

    /// Builds and executes a new L2 block using the provided payload attributes.
    ///
    /// This method performs the complete block building and execution process in a stateless
    /// manner, dynamically retrieving required state data via the trie database and producing
    /// a fully executed block with receipts and state commitments.
    ///
    /// # Arguments
    /// * `attrs` - Payload attributes containing transactions and block metadata
    ///
    /// # Returns
    /// * `Ok(BlockBuildingOutcome)` - Successfully built and executed block with receipts
    /// * `Err(ExecutorError)` - Block building or execution failure
    ///
    /// # Errors
    /// This method can fail due to various conditions:
    ///
    /// ## Input Validation Errors
    /// - [`ExecutorError::MissingGasLimit`]: Gas limit not provided in attributes
    /// - [`ExecutorError::MissingTransactions`]: Transaction list not provided
    /// - [`ExecutorError::MissingEIP1559Params`]: Required fee parameters missing (post-Holocene)
    /// - [`ExecutorError::MissingParentBeaconBlockRoot`]: Beacon root missing (post-Dencun)
    ///
    /// ## Execution Errors
    /// - [`ExecutorError::BlockGasLimitExceeded`]: Cumulative gas exceeds block limit
    /// - [`ExecutorError::UnsupportedTransactionType`]: Unknown transaction type encountered
    /// - [`ExecutorError::ExecutionError`]: EVM-level execution failures
    ///
    /// ## State Access Errors
    /// - [`ExecutorError::TrieDBError`]: State tree access or proof verification failures
    /// - Missing account data in witness
    /// - Invalid Merkle proofs
    ///
    /// ## Data Integrity Errors
    /// - [`ExecutorError::Recovery`]: Transaction signature recovery failures
    /// - [`ExecutorError::RLPError`]: Data encoding/decoding errors
    ///
    /// # Block Building Process
    ///
    /// The block building process follows these steps:
    ///
    /// 1. **Environment Setup**: Configure EVM environment with proper gas settings
    /// 2. **Witness Hinting**: Send payload witness hints to optimize state access
    /// 3. **Transaction Execution**: Execute each transaction in order with state updates
    /// 4. **Receipt Generation**: Generate execution receipts for all transactions
    /// 5. **State Commitment**: Compute final state roots and output commitments
    /// 6. **Block Assembly**: Assemble complete block with header and execution results
    ///
    /// # Stateless Execution Details
    ///
    /// Unlike traditional execution engines, this builder:
    /// - Resolves state access via Merkle proofs instead of database lookups
    /// - Validates all state access against cryptographic witnesses
    /// - Produces deterministic results independent of execution environment
    /// - Enables verification without full state replication
    ///
    /// # Performance Considerations
    ///
    /// - State access latency depends on proof verification overhead
    /// - Memory usage scales with witness size rather than full state
    /// - CPU overhead from cryptographic proof verification
    /// - I/O patterns optimized through trie hinter guidance
    pub fn build_block(
        &mut self,
        attrs: OpPayloadAttributes,
    ) -> ExecutorResult<BlockBuildingOutcome>
    where
        for<'b> IF::Inspector: Inspector<Evm::Context<&'b mut State<&'b mut TrieDB<P, H>>>>,
    {
        // Step 1. Set up the execution environment.
        let (base_fee_params, min_base_fee) = Self::active_base_fee_params(
            self.config,
            self.trie_db.parent_block_header(),
            attrs.payload_attributes.timestamp,
        )?;
        let evm_env = self.evm_env(
            self.config.spec_id(attrs.payload_attributes.timestamp),
            self.trie_db.parent_block_header(),
            &attrs,
            &base_fee_params,
            min_base_fee,
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
            block_number = %block_env.number,
            block_timestamp = %block_env.timestamp,
            block_gas_limit = block_env.gas_limit,
            transactions = attrs.transactions.as_ref().map_or(0, |txs| txs.len()),
            "Beginning block building."
        );

        let transactions = attrs
            .recovered_transactions_with_encoded()
            .collect::<Result<Vec<_>, RecoveryError>>()
            .map_err(ExecutorError::Recovery)?;

        let ctx = OpBlockExecutionCtx {
            parent_hash,
            parent_beacon_block_root: attrs.payload_attributes.parent_beacon_block_root,
            extra_data: Default::default(),
        };

        let (bundle, ex_result) = {
            let mut state = State::builder()
                .with_database(&mut self.trie_db)
                .with_bundle_update()
                .without_state_clear()
                .build();

            let ex_result = match &self.inspector_factory {
                // When no custom inspector factory is provided, use the standard create_evm().
                None => {
                    // Step 2. Create the executor, using the trie database.
                    let evm = self.factory.evm_factory().create_evm(&mut state, evm_env);

                    // Step 3. Execute the block containing the transactions within the payload attributes.
                    let executor = self.factory.create_executor(evm, ctx);
                    executor.execute_block(transactions.iter())?
                }
                // When a custom inspector factory is provided, we need to use raw pointers
                // to work around the borrow checker's limitations with generic types.
                //
                // NOTE: With a generic IF::Inspector, Rust cannot prove that the mutable
                // borrow of state ends when the EVM is dropped. We use raw pointers to work
                // around this. This is safe because:
                // 1. The state is valid for the entire duration of EVM usage
                // 2. The EVM is consumed by execute_block before we access state again
                // 3. No other references to state exist during EVM usage
                Some(factory) => {
                    let state_ptr = &mut state as *mut _;
                    let inspector = factory.create();

                    // Step 2. Create the executor, using the trie database.
                    // SAFETY: state is valid and not accessed elsewhere during EVM usage.
                    // The EVM is consumed by execute_block, ending the borrow.
                    let evm = self.factory.evm_factory().create_evm_with_inspector(
                        unsafe { &mut *state_ptr },
                        evm_env,
                        inspector,
                    );

                    // Step 3. Execute the block containing the transactions within the payload attributes.
                    let executor = self.factory.create_executor(evm, ctx);
                    executor.execute_block(transactions.iter())?
                }
            };

            // Step 4. Merge state transitions and seal the block.
            state.merge_transitions(BundleRetention::Reverts);
            let bundle = state.take_bundle();
            (bundle, ex_result)
        };

        info!(
            target: "block_builder",
            gas_used = ex_result.gas_used,
            gas_limit = block_env.gas_limit,
            "Finished block building. Beginning sealing job."
        );
        let header = self.seal_block(&attrs, parent_hash, &block_env, &ex_result, bundle)?;

        info!(
            target: "block_builder",
            number = header.number,
            hash = ?header.seal(),
            state_root = ?header.state_root,
            transactions_root = ?header.transactions_root,
            receipts_root = ?header.receipts_root,
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

    #[rstest]
    #[tokio::test]
    async fn test_statelessly_execute_block(
        #[base_dir = "./testdata"]
        #[files("*.tar.gz")]
        path: PathBuf,
    ) {
        run_test_fixture(path).await;
    }
}
