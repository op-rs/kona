//! Consolidation phase of the interop proof program.

use super::FaultProofProgramError;
use crate::interop::util::fetch_output_block_hash;
use alloc::{sync::Arc, vec::Vec};
use core::fmt::Debug;
use kona_preimage::{HintWriterClient, PreimageOracleClient};
use kona_proof::{l2::OracleL2ChainProvider, CachingOracle};
use kona_proof_interop::{
    BootInfo, HintType, OracleInteropProvider, PreState, SuperchainConsolidator,
};
use maili_registry::{HashMap, ROLLUP_CONFIGS};
use tracing::info;

/// Executes the consolidation phase of the interop proof with the given [PreimageOracleClient] and
/// [HintWriterClient].
///
/// This phase is responsible for checking the dependencies between [OptimisticBlock]s in the
/// superchain and ensuring that all dependencies are satisfied.
///
/// [OptimisticBlock]: kona_proof_interop::OptimisticBlock
pub(crate) async fn consolidate_dependencies<P, H>(
    oracle: Arc<CachingOracle<P, H>>,
    mut boot: BootInfo,
) -> Result<(), FaultProofProgramError>
where
    P: PreimageOracleClient + Send + Sync + Debug + Clone,
    H: HintWriterClient + Send + Sync + Debug + Clone,
{
    let provider = OracleInteropProvider::new(oracle.clone(), boot.agreed_pre_state.clone());

    info!(target: "client_interop", "Deriving local-safe headers from prestate");

    // Ensure that the pre-state is a transition state.
    let PreState::TransitionState(ref transition_state) = boot.agreed_pre_state else {
        return Err(FaultProofProgramError::StateTransitionFailed);
    };

    let block_hashes = transition_state
        .pending_progress
        .iter()
        .zip(transition_state.pre_state.output_roots.iter())
        .map(|(optimistic_block, pre_state)| (pre_state, optimistic_block.block_hash))
        .collect::<HashMap<_, _>>();

    let mut headers = Vec::with_capacity(block_hashes.len());
    let mut l2_providers = HashMap::default();
    for (pre, block_hash) in block_hashes {
        // Fetch the safe head's block hash for the given L2 chain ID.
        let safe_head_hash =
            fetch_output_block_hash(oracle.as_ref(), pre.output_root, pre.chain_id).await?;

        // Send hints for the L2 block data in the pending progress. This is an important step,
        // because non-canonical blocks within the pending progress will not be able to be fetched
        // by the host through the traditional means. If the block is determined to not be canonical
        // by the host, it will re-execute it and store the required preimages to complete
        // deposit-only re-execution. If the block is determined to be canonical, the host will
        // no-op, and fetch preimages through the traditional route as needed.
        HintType::L2BlockData
            .with_data(&[
                safe_head_hash.as_slice(),
                block_hash.as_slice(),
                pre.chain_id.to_be_bytes().as_slice(),
            ])
            .send(oracle.as_ref())
            .await?;

        let header = provider.header_by_hash(pre.chain_id, block_hash).await?;
        headers.push((pre.chain_id, header.seal(block_hash)));

        let rollup_config = ROLLUP_CONFIGS
            .get(&pre.chain_id)
            .or_else(|| boot.rollup_configs.get(&pre.chain_id))
            .ok_or(FaultProofProgramError::MissingRollupConfig(pre.chain_id))?;

        let mut provider = OracleL2ChainProvider::new(
            safe_head_hash,
            Arc::new(rollup_config.clone()),
            oracle.clone(),
        );
        provider.set_chain_id(Some(pre.chain_id));
        l2_providers.insert(pre.chain_id, provider);
    }

    info!(target: "client_interop", "Loaded {} local-safe headers", headers.len());

    // Consolidate the superchain
    SuperchainConsolidator::new(&mut boot, provider, l2_providers, headers).consolidate().await?;

    // Transition to the Super Root at the next timestamp.
    let post = boot
        .agreed_pre_state
        .transition(None)
        .ok_or(FaultProofProgramError::StateTransitionFailed)?;
    let post_commitment = post.hash();

    // Ensure that the post-state matches the claimed post-state.
    if post_commitment != boot.claimed_post_state {
        return Err(FaultProofProgramError::InvalidClaim(boot.claimed_post_state, post_commitment));
    }

    Ok(())
}
