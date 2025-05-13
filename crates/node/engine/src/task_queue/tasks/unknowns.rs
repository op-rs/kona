//! Utility function for initializing unknown engine state.

use crate::{EngineClient, EngineState};
use alloy_eips::eip1898::BlockNumberOrTag;
use kona_genesis::RollupConfig;
use std::sync::Arc;

/// Initialize Unknown Engine State.
///
/// For each unknown head, attempt to fetch it from the [`EngineClient`].
pub async fn init_unknowns(
    state: &mut EngineState,
    client: Arc<EngineClient>,
    cfg: &Arc<RollupConfig>,
) {
    // Initialize the unsafe head if it is not already set.
    if state.unsafe_head.block_info.hash.is_zero() {
        let head = match client.l2_block_info_by_label(BlockNumberOrTag::Pending).await {
            Ok(Some(h)) => h,
            Ok(None) => {
                warn!(target: "engine", "No pending head found.");
                return;
            }
            Err(e) => {
                warn!(target: "engine", ?e, "Error fetching pending head.");
                return;
            }
        };
        state.set_unsafe_head(head);
    }

    // Initialize the finalized head if it is not already set.
    if state.finalized_head.block_info.hash.is_zero() {
        let head = match client.l2_block_info_by_label(BlockNumberOrTag::Finalized).await {
            Ok(Some(h)) => h,
            Ok(None) => {
                debug!(target: "engine", "No finalized head found. Trying to use genesis.");

                let Ok(Some(head)) = client
                    .l2_block_info_by_label(BlockNumberOrTag::Number(cfg.genesis.l2.number))
                    .await
                else {
                    warn!(target: "engine", "Impossible to fetch genesis head");
                    return;
                };

                head
            }
            Err(e) => {
                warn!(target: "engine", ?e, "Error fetching finalized head");
                return;
            }
        };
        state.set_finalized_head(head);
    }

    // Initialize the safe head if it is not already set.
    if state.safe_head.block_info.hash.is_zero() {
        let head = match client.l2_block_info_by_label(BlockNumberOrTag::Safe).await {
            Ok(Some(h)) => h,
            Ok(None) => {
                debug!(target: "engine", "No safe head found. Using the finalized head");
                state.finalized_head()
            }
            Err(e) => {
                warn!(target: "engine", ?e, "Error fetching safe head");
                return;
            }
        };
        state.set_safe_head(head);
    }

    // If the cross unsafe head is not set, set it to the safe head.
    if state.cross_unsafe_head.block_info.hash.is_zero() {
        state.set_cross_unsafe_head(state.safe_head);
    }
    // If the local safe head is not set, set it to the safe head.
    if state.local_safe_head.block_info.hash.is_zero() {
        state.set_local_safe_head(state.safe_head);
    }
}
