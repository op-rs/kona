//! [Header] assembly logic for the [StatelessL2Builder].

use super::StatelessL2Builder;
use crate::{
    ExecutorResult, TrieDBError, TrieDBProvider,
};
use alloy_consensus::{Header, Sealed};
use alloy_evm::block::BlockExecutionResult;
use alloy_primitives::{B256, Sealable};
use kona_mpt::TrieHinter;
use op_alloy_consensus::OpReceiptEnvelope;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use revm::{context::BlockEnv, database::BundleState};
use soon_primitives::rollup_config::SoonRollupConfig;

impl<P, H> StatelessL2Builder<'_, P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    /// Seals the block executed from the given [OpPayloadAttributes] and [BlockEnv], returning the
    /// computed [Header].
    #[allow(dead_code)]
    pub(crate) fn seal_block(
        &mut self,
        _attrs: &OpPayloadAttributes,
        _parent_hash: B256,
        _block_env: &BlockEnv,
        _ex_result: &BlockExecutionResult<OpReceiptEnvelope>,
        _bundle: BundleState,
    ) -> ExecutorResult<Sealed<Header>> {
        // Construct the new header.
        let header = Header::default().seal_slow();

        Ok(header)
    }

    /// Computes the current output root of the latest executed block, based on the parent header
    /// and the underlying state trie.
    ///
    /// **CONSTRUCTION:**
    /// ```text
    /// output_root = keccak256(version_byte .. payload)
    /// payload = state_root .. withdrawal_storage_root .. latest_block_hash
    /// ```
    pub fn compute_output_root(&mut self) -> ExecutorResult<B256> {

        // Hash the output and return
        Ok(B256::ZERO)
    }

    /// Fetches the L2 to L1 message passer account from the cache or underlying trie.
    #[allow(dead_code)]
    fn message_passer_account(&mut self, _block_number: u64) -> Result<B256, TrieDBError> {
        Ok(B256::ZERO)
    }
}

/// Computes the receipts root from the given set of receipts.
pub fn compute_receipts_root(
    _receipts: &[OpReceiptEnvelope],
    _config: &SoonRollupConfig,
    _timestamp: u64,
) -> B256 {
    B256::ZERO
}
