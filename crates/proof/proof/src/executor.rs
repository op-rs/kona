//! An executor constructor.

use alloc::boxed::Box;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_driver::Executor;
use soon_primitives::rollup_config::SoonRollupConfig;
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use kona_executor::{StatelessL2Builder, TrieDBProvider};

/// An executor wrapper type.
#[derive(Debug)]
pub struct KonaExecutor<'a, P, H>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
{
    /// The rollup config for the executor.
    rollup_config: &'a SoonRollupConfig,
    /// The trie provider for the executor.
    trie_provider: P,
    /// The trie hinter for the executor.
    trie_hinter: H,
    /// The executor.
    inner: Option<StatelessL2Builder<'a, P, H>>,
}

impl<'a, P, H> KonaExecutor<'a, P, H>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
{
    /// Creates a new executor.
    pub const fn new(
        rollup_config: &'a SoonRollupConfig,
        trie_provider: P,
        trie_hinter: H,
        inner: Option<StatelessL2Builder<'a, P, H>>,
    ) -> Self {
        Self { rollup_config, trie_provider, trie_hinter, inner }
    }
}

#[async_trait]
impl<P, H> Executor for KonaExecutor<'_, P, H>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
{
    type Error = kona_executor::ExecutorError;

    /// Waits for the executor to be ready.
    async fn wait_until_ready(&mut self) {
        /* no-op for the kona executor */
        /* This is used when an engine api is used instead of a stateless block executor */
    }

    /// Updates the safe header.
    ///
    /// Since the L2 block executor is stateless, on an update to the safe head,
    /// a new executor is created with the updated header.
    fn update_safe_head(&mut self, header: Sealed<Header>) {
        self.inner = Some(StatelessL2Builder::new(
            self.rollup_config,
            self.trie_provider.clone(),
            self.trie_hinter.clone(),
            header,
        ));
    }

    /// Execute the given payload attributes.
    async fn execute_payload(
        &mut self,
        _attributes: OpPayloadAttributes,
    ) -> Result<(), Self::Error> {
        // self.inner.as_mut().map_or_else(
        //     || Err(kona_executor::ExecutorError::MissingExecutor),
        //     |e| e.build_block(attributes),
        // )
        Ok(())
    }

    /// Computes the output root.
    fn compute_output_root(&mut self) -> Result<B256, Self::Error> {
        // self.inner.as_mut().map_or_else(
        //     || Err(kona_executor::ExecutorError::MissingExecutor),
        //     |e| e.compute_output_root(),
        // )
        Ok(B256::ZERO)
    }
}
