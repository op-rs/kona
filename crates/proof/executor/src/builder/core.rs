//! The [StatelessL2Builder] is a block builder that pulls state from a [TrieDB] during execution.

use crate::{ExecutorResult, TrieDB, TrieDBProvider};
use alloy_consensus::{Header, Sealed};
use alloy_evm::block::BlockExecutionResult;
use soon_primitives::rollup_config::SoonRollupConfig;
use kona_mpt::TrieHinter;
use op_alloy_consensus::{OpReceiptEnvelope};
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// The [`StatelessL2Builder`] is an OP Stack block builder that traverses a merkle patricia trie
/// via the [`TrieDB`] during execution.
#[derive(Debug)]
pub struct StatelessL2Builder<'a, P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    /// The [SoonRollupConfig].
    #[allow(dead_code)]
    pub(crate) config: &'a SoonRollupConfig,
    /// The inner trie database.
    #[allow(dead_code)]
    pub(crate) trie_db: TrieDB<P, H>,
    /// The executor factory, used to create new [`op_revm::OpEvm`] instances for block building
    /// routines.
    #[allow(dead_code)]
    pub(crate) factory: Option<bool>,
}

impl<'a, P, H> StatelessL2Builder<'a, P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    /// Creates a new [StatelessL2Builder] instance.
    pub fn new(
        config: &'a SoonRollupConfig,
        provider: P,
        hinter: H,
        parent_header: Sealed<Header>,
    ) -> Self {
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        Self { config, trie_db, factory:None }
    }

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    pub fn build_block(
        &mut self,
        _attrs: OpPayloadAttributes,
    ) -> ExecutorResult<()> {

        Ok(())
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
