//! Sync start algorithm for the OP Stack rollup node.

use kona_derive::traits::ChainProvider;
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo};
use kona_providers_alloy::{
    AlloyChainProvider, AlloyChainProviderError, AlloyL2ChainProvider, AlloyL2ChainProviderError,
};

mod forkchoice;
pub use forkchoice::L2ForkchoiceState;

mod error;
pub use error::SyncStartError;

/// [RECOVER_MIN_SEQ_WINDOWS] is the number of sequence windows between the unsafe head L1 origin,
/// and the finalized block, while finality is still at genesis, that need to elapse to
/// heuristically recover from a missing forkchoice state.
///
/// A healthy node should not need to recover from this state, since auto-sequencing begins after a
/// full sequence window has elapsed with no safe head advancement.
const RECOVER_MIN_SEQ_WINDOWS: u64 = 14;

/// [MAX_REORG_SEQ_WINDOWS] is the maximum number of sequence windows that we allow for reorgs to
/// occur before we consider the node to be in a corrupted state.
const MAX_REORG_SEQ_WINDOWS: u64 = 5;

/// Searches for the latest [`L2ForkchoiceState`] that we can use to start the sync process with.
///
///   - The *unsafe L2 block*: This is the highest L2 block whose L1 origin is a *plausible*
///     extension of the canonical L1 chain (as known to the op-node).
///   - The *safe L2 block*: This is the highest L2 block whose epoch's sequencing window is
///     complete within the canonical L1 chain (as known to the op-node).
///   - The *finalized L2 block*: This is the L2 block which is known to be fully derived from
///     finalized L1 block data.
///
/// Plausible: meaning that the blockhash of the L2 block's L1 origin
/// (as reported in the L1 Attributes deposit within the L2 block) is not canonical at another
/// height in the L1 chain, and the same holds for all its ancestors.
pub async fn find_starting_forkchoice(
    cfg: &RollupConfig,
    l1_provider: &mut AlloyChainProvider,
    l2_provider: &mut AlloyL2ChainProvider,
) -> Result<L2ForkchoiceState, SyncStartError> {
    let mut current_fc = L2ForkchoiceState::current(cfg, l2_provider).await?;
    info!(
        target: "sync_start",
        unsafe = %current_fc.un_safe.block_info.number,
        safe = %current_fc.safe.block_info.number,
        finalized = %current_fc.finalized.block_info.number,
        "Loaded current L2 EL forkchoice state"
    );

    // Check if we can recover from a finality-less sync state
    if should_recover_from_finality_less_sync(&current_fc, cfg) {
        warn!(
            target: "sync_start",
            "Attempting recovery from sync state without finality. Heads set to {:?}",
            current_fc.un_safe
        );
        return Ok(L2ForkchoiceState {
            un_safe: current_fc.un_safe,
            safe: current_fc.un_safe,
            finalized: current_fc.un_safe,
        });
    }

    // Check if the execution engine is corrupted
    if current_fc.un_safe.block_info.number < current_fc.finalized.block_info.number ||
        current_fc.un_safe.block_info.number < current_fc.safe.block_info.number
    {
        warn!(
            target: "sync_start",
            "Unsafe head is behind known finalized / safe blocks. Attempting recovery from corrupt EL forkchoice state."
        );
        warn!(target: "sync_start", "Corrupted Forkchoice State: {current_fc}");

        return Ok(L2ForkchoiceState {
            un_safe: current_fc.un_safe,
            safe: current_fc.un_safe,
            finalized: current_fc.un_safe,
        });
    }

    // Traverse backwards from unsafe head to find the starting forkchoice state
    traverse_l2(cfg, l1_provider, l2_provider, &mut current_fc).await?;
    Ok(current_fc)
}

/// Checks if we need to recover from a state where finality is still at genesis.
fn should_recover_from_finality_less_sync(
    current_fc: &L2ForkchoiceState,
    cfg: &RollupConfig,
) -> bool {
    let genesis_hash = cfg.genesis.l2.hash;
    let min_l1_block_threshold =
        cfg.genesis.l1.number + (RECOVER_MIN_SEQ_WINDOWS * cfg.seq_window_size);

    current_fc.finalized.block_info.hash == genesis_hash &&
        current_fc.safe.block_info.hash == genesis_hash &&
        current_fc.un_safe.block_info.number > cfg.genesis.l2.number &&
        current_fc.un_safe.l1_origin.number > min_l1_block_threshold
}

/// Traverses the L2 chain, starting from the `current_fc` [`L2ForkchoiceState`],
/// to find the starting forkchoice.
async fn traverse_l2(
    cfg: &RollupConfig,
    l1_provider: &mut AlloyChainProvider,
    l2_provider: &mut AlloyL2ChainProvider,
    current_fc: &mut L2ForkchoiceState,
) -> Result<(), SyncStartError> {
    // Start from unsafe head
    let original_unsafe = current_fc.un_safe;
    let mut current_block = current_fc.un_safe;
    let mut highest_canonical_l2: Option<L2BlockInfo> = None;
    let mut l1_block: Option<BlockInfo> = None;
    let mut ahead = false;
    let mut ready = false;

    // Traverse backwards from unsafe head towards finalized head
    loop {
        l1_block = fetch_l1_block(l1_provider, &current_block, l1_block, &mut ahead).await?;

        // Don't walk past genesis
        check_genesis_boundaries(cfg, &current_block, ahead, l1_block)?;

        // Check if the current block is the finalized block, and if it is, check if the block
        // hashes match.
        check_finalized_block_mismatch(&current_block, current_fc)?;

        // Update highest canonical L2 block
        update_highest_canonical_l2(
            cfg,
            &original_unsafe,
            ahead,
            l1_block,
            &current_block,
            &mut highest_canonical_l2,
            current_fc,
        );

        // Check if we've found a suitable safe head
        if is_ready_for_sync(&current_block, current_fc, highest_canonical_l2, cfg.seq_window_size)
        {
            ready = true;
        }

        // Stop if we've reached the finalized block
        if current_block.block_info.number == current_fc.finalized.block_info.number {
            info!(target: "sync_start", "Reached finalized L2 head, returning immediately.");
            current_fc.safe = current_block;
            break;
        }

        // Fetch parent block for next iteration
        let parent = l2_provider
            .block_info_by_id(current_block.block_info.parent_hash.into())
            .await?
            .ok_or(AlloyL2ChainProviderError::BlockNotFound(current_block.block_info.number))?;

        // Validate L1 origin relationship
        validate_l1_origin_relationship(&current_block, &parent, l1_block)?;

        current_block = parent;
        if ready {
            current_fc.safe = current_block;
            break;
        }
    }

    Ok(())
}

/// Fetches the L1 block corresponding to the current L2 block's origin.
async fn fetch_l1_block(
    l1_provider: &mut AlloyChainProvider,
    current_block: &L2BlockInfo,
    current_l1_block: Option<BlockInfo>,
    ahead: &mut bool,
) -> Result<Option<BlockInfo>, SyncStartError> {
    let l1_hash = current_block.l1_origin.hash;

    // If current L1 block's parent is the same as current L2 block's origin
    if current_l1_block.as_ref().is_some_and(|b| b.parent_hash == l1_hash) {
        let (new_l1_block, _) = l1_provider.block_info_and_transactions_by_hash(l1_hash).await?;

        info!(
            target: "sync_start",
            current = current_l1_block.map(|b| b.number).unwrap_or_default(),
            next = new_l1_block.number,
            l2 = current_block.block_info.number,
            "Walking back L1 block by hash",
        );

        *ahead = false;
        Ok(Some(new_l1_block))
    }
    // If we don't have an L1 block yet or the current block's origin doesn't match current L1 block
    else if current_l1_block.is_none() ||
        current_l1_block.is_some_and(|l1| l1.hash != current_block.l1_origin.hash)
    {
        let resp = l1_provider.block_info_and_transactions_by_hash(l1_hash).await;
        let not_found = matches!(resp, Err(AlloyChainProviderError::BlockNotFound(_)));
        let (new_l1_block, _) = resp?;

        *ahead = not_found;
        Ok(Some(new_l1_block))
    } else {
        Ok(current_l1_block)
    }
}

/// Checks if the current block is the L2 genesis block. If it is, ensures that both the L1
/// and L2 genesis are consistent.
fn check_genesis_boundaries(
    cfg: &RollupConfig,
    current_block: &L2BlockInfo,
    ahead: bool,
    l1_block: Option<BlockInfo>,
) -> Result<(), SyncStartError> {
    if current_block.block_info.number == cfg.genesis.l2.number {
        if current_block.block_info.hash != cfg.genesis.l2.hash {
            return Err(SyncStartError::InvalidL2GenesisHash(
                cfg.genesis.l2.hash,
                current_block.block_info.hash,
            ));
        }
        if !ahead && l1_block.as_ref().is_some_and(|b| b.hash != cfg.genesis.l1.hash) {
            return Err(SyncStartError::InvalidL1GenesisHash(
                cfg.genesis.l1.hash,
                l1_block.map(|b| b.hash).unwrap_or_default(),
            ));
        }
    }
    Ok(())
}

/// Checks if the current block matches the finalized block number but has a different hash.
fn check_finalized_block_mismatch(
    current_block: &L2BlockInfo,
    forkchoice: &L2ForkchoiceState,
) -> Result<(), SyncStartError> {
    (current_block.block_info.number != forkchoice.finalized.block_info.number ||
        current_block.block_info.hash == forkchoice.finalized.block_info.hash)
        .then_some(())
        .ok_or(SyncStartError::MismatchedFinalizedBlock(
            current_block.block_info.hash,
            forkchoice.finalized.block_info.hash,
        ))
}

/// Updates the highest canonical L2 block based on current traversal state.
fn update_highest_canonical_l2(
    cfg: &RollupConfig,
    original_unsafe: &L2BlockInfo,
    ahead: bool,
    l1_block: Option<BlockInfo>,
    current_block: &L2BlockInfo,
    highest_canonical_l2: &mut Option<L2BlockInfo>,
    current_fc: &mut L2ForkchoiceState,
) {
    if current_fc.un_safe == L2BlockInfo::default() {
        current_fc.un_safe = *current_block;

        if current_block.l1_origin.number + (MAX_REORG_SEQ_WINDOWS * cfg.seq_window_size) <
            original_unsafe.l1_origin.number
        {
            error!(
                target: "sync_start",
                "Critical failure. Traversed back to L2 block #{}, but L1 origin (#{}) is too far behind relative to the original unsafe head (#{} - L1 origin: {}).",
                current_block.block_info.number,
                current_block.l1_origin.number,
                original_unsafe.block_info.number,
                original_unsafe.l1_origin.number
            );
        }
    }

    if ahead {
        // Discard previous candidate if we're ahead of L1 head
        *highest_canonical_l2 = None;
    } else if l1_block.as_ref().is_some_and(|b| b.hash == current_block.l1_origin.hash) {
        // Update highest canonical L2 block if L1 origin matches canonical chain
        if highest_canonical_l2.is_none() {
            *highest_canonical_l2 = Some(*current_block);
        }
    } else {
        // L1 origin is neither ahead nor canonical, discard candidate
        current_fc.un_safe = L2BlockInfo::default();
        *highest_canonical_l2 = None;
    }
}

/// Checks if we've found a suitable safe head to start sync.
fn is_ready_for_sync(
    current_block: &L2BlockInfo,
    forkchoice: &L2ForkchoiceState,
    highest_canonical_l2: Option<L2BlockInfo>,
    seq_window_size: u64,
) -> bool {
    current_block.block_info.number <= forkchoice.safe.block_info.number &&
        current_block.l1_origin.number + seq_window_size <
            highest_canonical_l2.map(|b| b.l1_origin.number).unwrap_or_default() &&
        current_block.seq_num == 0
}

/// Validates the L1 origin relationship between a block and its parent.
fn validate_l1_origin_relationship(
    current_block: &L2BlockInfo,
    parent: &L2BlockInfo,
    l1_block: Option<BlockInfo>,
) -> Result<(), SyncStartError> {
    if parent.l1_origin != current_block.l1_origin {
        // Sanity check the L1 origin block number
        if parent.l1_origin.number + 1 != current_block.l1_origin.number {
            return Err(SyncStartError::L1OriginMismatch);
        }

        // Sanity check that the sequence number is 0 if L1 origin changed
        if current_block.seq_num != 0 {
            return Err(SyncStartError::NonZeroSequenceNumber);
        }

        // Check that parent L1 origin is consistent with canonical chain
        if l1_block.as_ref().is_some_and(|b| {
            b.hash == current_block.l1_origin.hash && b.parent_hash != parent.l1_origin.hash
        }) {
            return Err(SyncStartError::L1OriginMismatch);
        }
    } else if parent.seq_num + 1 != current_block.seq_num {
        return Err(SyncStartError::InconsistentSequenceNumber);
    }

    Ok(())
}
