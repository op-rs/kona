//! The internal state of the engine controller.

use crate::Metrics;
use alloy_rpc_types_engine::ForkchoiceState;
use kona_protocol::L2BlockInfo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error that occurs when constructing or updating an [`EngineSyncState`].
///
/// This error type represents invalid state configurations that violate the safety level
/// invariants. The safety levels must maintain the ordering:
/// `finalized <= safe <= local_safe <= cross_unsafe <= unsafe`
///
/// The error contains full debugging information including the previous state,
/// the update that was attempted, and the resulting block numbers showing where
/// the invariant violation occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error(
    "Invalid sync state: expected finalized ({finalized}) <= safe ({safe}) <= \
    local_safe ({local_safe}) <= cross_unsafe ({cross_unsafe}) <= unsafe ({unsafe_head})"
)]
pub struct InvalidEngineSyncStateError {
    /// The previous sync state before the update was attempted, if any.
    pub previous_state: Option<EngineSyncState>,
    /// The update that was attempted.
    pub update: EngineSyncStateUpdate,
    /// The resulting finalized head block number.
    pub finalized: u64,
    /// The resulting safe head block number.
    pub safe: u64,
    /// The resulting local safe head block number.
    pub local_safe: u64,
    /// The resulting cross unsafe head block number.
    pub cross_unsafe: u64,
    /// The resulting unsafe head block number.
    pub unsafe_head: u64,
}

/// The synchronization state of the execution layer across different safety levels.
///
/// Tracks block progression through various stages of verification and finalization,
/// from initial unsafe blocks received via P2P to fully finalized blocks derived from
/// finalized L1 data. Each level represents increasing confidence in the block's validity.
///
/// # Safety Levels
///
/// The state tracks blocks at different safety levels, listed from least to most safe:
///
/// 1. **Unsafe** - Most recent blocks from P2P network (unverified)
/// 2. **Cross-unsafe** - Unsafe blocks with cross-layer verification
/// 3. **Local-safe** - Derived from L1 data, completed span-batch
/// 4. **Safe** - Cross-verified with safe L1 dependencies
/// 5. **Finalized** - Derived from finalized L1 data only
///
/// See the [OP Stack specifications](https://specs.optimism.io) for detailed safety definitions.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineSyncState {
    /// Most recent block found on the P2P network (lowest safety level).
    unsafe_head: L2BlockInfo,
    /// Cross-verified unsafe head (equal to unsafe_head pre-interop).
    cross_unsafe_head: L2BlockInfo,
    /// Derived from L1 data as a completed span-batch, but not yet cross-verified.
    local_safe_head: L2BlockInfo,
    /// Derived from L1 data and cross-verified to have safe L1 dependencies.
    safe_head: L2BlockInfo,
    /// Derived from finalized L1 data with only finalized dependencies (highest safety level).
    finalized_head: L2BlockInfo,
}

impl EngineSyncState {
    /// Returns the current unsafe head.
    pub const fn unsafe_head(&self) -> L2BlockInfo {
        self.unsafe_head
    }

    /// Returns the current cross-verified unsafe head.
    pub const fn cross_unsafe_head(&self) -> L2BlockInfo {
        self.cross_unsafe_head
    }

    /// Returns the current local safe head.
    pub const fn local_safe_head(&self) -> L2BlockInfo {
        self.local_safe_head
    }

    /// Returns the current safe head.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.safe_head
    }

    /// Returns the current finalized head.
    pub const fn finalized_head(&self) -> L2BlockInfo {
        self.finalized_head
    }

    /// Creates a `ForkchoiceState`
    ///
    /// - `head_block` = `unsafe_head`
    /// - `safe_block` = `safe_head`
    /// - `finalized_block` = `finalized_head`
    ///
    /// If the block info is not yet available, the default values are used.
    pub const fn create_forkchoice_state(&self) -> ForkchoiceState {
        ForkchoiceState {
            head_block_hash: self.unsafe_head.hash(),
            safe_block_hash: self.safe_head.hash(),
            finalized_block_hash: self.finalized_head.hash(),
        }
    }

    /// Applies the update to the provided sync state, using the current state values if the update
    /// is not specified. Returns the new sync state.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting state would violate safety level invariants.
    /// The invariant requires: `finalized <= safe <= local_safe <= cross_unsafe <= unsafe`
    pub fn apply_update(
        self,
        sync_state_update: EngineSyncStateUpdate,
    ) -> Result<Self, Box<InvalidEngineSyncStateError>> {
        let new_state = Self {
            unsafe_head: sync_state_update.unsafe_head.unwrap_or(self.unsafe_head),
            cross_unsafe_head: sync_state_update
                .cross_unsafe_head
                .unwrap_or(self.cross_unsafe_head),
            local_safe_head: sync_state_update.local_safe_head.unwrap_or(self.local_safe_head),
            safe_head: sync_state_update.safe_head.unwrap_or(self.safe_head),
            finalized_head: sync_state_update.finalized_head.unwrap_or(self.finalized_head),
        };

        // Extract block numbers for validation
        let finalized = new_state.finalized_head.block_info.number;
        let safe = new_state.safe_head.block_info.number;
        let local_safe = new_state.local_safe_head.block_info.number;
        let cross_unsafe = new_state.cross_unsafe_head.block_info.number;
        let unsafe_head = new_state.unsafe_head.block_info.number;

        // Validate the full ordering: finalized <= safe <= local_safe <= cross_unsafe <= unsafe
        let is_valid = finalized <= safe &&
            safe <= local_safe &&
            local_safe <= cross_unsafe &&
            cross_unsafe <= unsafe_head;

        if !is_valid {
            return Err(Box::new(InvalidEngineSyncStateError {
                previous_state: Some(self),
                update: sync_state_update,
                finalized,
                safe,
                local_safe,
                cross_unsafe,
                unsafe_head,
            }));
        }

        // Update metrics after validation succeeds
        if let Some(unsafe_head) = sync_state_update.unsafe_head {
            Self::update_block_label_metric(
                Metrics::UNSAFE_BLOCK_LABEL,
                unsafe_head.block_info.number,
            );
        }
        if let Some(cross_unsafe_head) = sync_state_update.cross_unsafe_head {
            Self::update_block_label_metric(
                Metrics::CROSS_UNSAFE_BLOCK_LABEL,
                cross_unsafe_head.block_info.number,
            );
        }
        if let Some(local_safe_head) = sync_state_update.local_safe_head {
            Self::update_block_label_metric(
                Metrics::LOCAL_SAFE_BLOCK_LABEL,
                local_safe_head.block_info.number,
            );
        }
        if let Some(safe_head) = sync_state_update.safe_head {
            Self::update_block_label_metric(Metrics::SAFE_BLOCK_LABEL, safe_head.block_info.number);
        }
        if let Some(finalized_head) = sync_state_update.finalized_head {
            Self::update_block_label_metric(
                Metrics::FINALIZED_BLOCK_LABEL,
                finalized_head.block_info.number,
            );
        }

        Ok(new_state)
    }

    /// Updates a block label metric, keyed by the label.
    #[inline]
    fn update_block_label_metric(label: &'static str, number: u64) {
        kona_macros::set!(gauge, Metrics::BLOCK_LABELS, "label", label, number as f64);
    }
}

/// Specifies how to update the sync state of the engine.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSyncStateUpdate {
    /// Most recent block found on the p2p network
    pub unsafe_head: Option<L2BlockInfo>,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    pub cross_unsafe_head: Option<L2BlockInfo>,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    pub local_safe_head: Option<L2BlockInfo>,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    pub safe_head: Option<L2BlockInfo>,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    pub finalized_head: Option<L2BlockInfo>,
}

/// The chain state viewed by the engine controller.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineState {
    /// The sync state of the engine.
    pub sync_state: EngineSyncState,

    /// Whether or not the EL has finished syncing.
    pub el_sync_finished: bool,

    /// Track when the rollup node changes the forkchoice to restore previous
    /// known unsafe chain. e.g. Unsafe Reorg caused by Invalid span batch.
    /// This update does not retry except engine returns non-input error
    /// because engine may forgot backupUnsafeHead or backupUnsafeHead is not part
    /// of the chain.
    pub need_fcu_call_backup_unsafe_reorg: bool,
}

impl EngineState {
    /// Returns if consolidation is needed.
    ///
    /// [Consolidation] is only performed by a rollup node when the unsafe head
    /// is ahead of the safe head. When the two are equal, consolidation isn't
    /// required and the [`crate::BuildTask`] can be used to build the block.
    ///
    /// [Consolidation]: https://specs.optimism.io/protocol/derivation.html#l1-consolidation-payload-attributes-matching
    pub fn needs_consolidation(&self) -> bool {
        self.sync_state.safe_head() != self.sync_state.unsafe_head()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Metrics;
    use kona_protocol::BlockInfo;
    use metrics_exporter_prometheus::PrometheusBuilder;
    use rstest::rstest;

    impl EngineState {
        /// Set the unsafe head.
        pub fn set_unsafe_head(&mut self, unsafe_head: L2BlockInfo) {
            self.sync_state = self
                .sync_state
                .apply_update(EngineSyncStateUpdate {
                    unsafe_head: Some(unsafe_head),
                    ..Default::default()
                })
                .expect("valid sync state update");
        }

        /// Set the cross-verified unsafe head.
        pub fn set_cross_unsafe_head(&mut self, cross_unsafe_head: L2BlockInfo) {
            self.sync_state = self
                .sync_state
                .apply_update(EngineSyncStateUpdate {
                    cross_unsafe_head: Some(cross_unsafe_head),
                    ..Default::default()
                })
                .expect("valid sync state update");
        }

        /// Set the local safe head.
        pub fn set_local_safe_head(&mut self, local_safe_head: L2BlockInfo) {
            self.sync_state = self
                .sync_state
                .apply_update(EngineSyncStateUpdate {
                    local_safe_head: Some(local_safe_head),
                    ..Default::default()
                })
                .expect("valid sync state update");
        }

        /// Set the safe head.
        pub fn set_safe_head(&mut self, safe_head: L2BlockInfo) {
            self.sync_state = self
                .sync_state
                .apply_update(EngineSyncStateUpdate {
                    safe_head: Some(safe_head),
                    ..Default::default()
                })
                .expect("valid sync state update");
        }

        /// Set the finalized head.
        pub fn set_finalized_head(&mut self, finalized_head: L2BlockInfo) {
            self.sync_state = self
                .sync_state
                .apply_update(EngineSyncStateUpdate {
                    finalized_head: Some(finalized_head),
                    ..Default::default()
                })
                .expect("valid sync state update");
        }
    }

    #[rstest]
    #[case::set_unsafe(EngineState::set_unsafe_head, Metrics::UNSAFE_BLOCK_LABEL, 1)]
    #[case::set_cross_unsafe(
        EngineState::set_cross_unsafe_head,
        Metrics::CROSS_UNSAFE_BLOCK_LABEL,
        2
    )]
    #[case::set_local_safe(EngineState::set_local_safe_head, Metrics::LOCAL_SAFE_BLOCK_LABEL, 3)]
    #[case::set_safe_head(EngineState::set_safe_head, Metrics::SAFE_BLOCK_LABEL, 4)]
    #[case::set_finalized_head(EngineState::set_finalized_head, Metrics::FINALIZED_BLOCK_LABEL, 5)]
    #[cfg(feature = "metrics")]
    fn test_chain_label_metrics(
        #[case] set_fn: impl Fn(&mut EngineState, L2BlockInfo),
        #[case] label_name: &str,
        #[case] number: u64,
    ) {
        let handle = PrometheusBuilder::new().install_recorder().unwrap();
        crate::Metrics::init();

        let mut state = EngineState::default();
        set_fn(
            &mut state,
            L2BlockInfo {
                block_info: BlockInfo { number, ..Default::default() },
                ..Default::default()
            },
        );

        assert!(handle.render().contains(
            format!("kona_node_block_labels{{label=\"{label_name}\"}} {number}").as_str()
        ));
    }

    fn block_info(number: u64) -> L2BlockInfo {
        L2BlockInfo { block_info: BlockInfo { number, ..Default::default() }, ..Default::default() }
    }

    #[test]
    fn test_apply_update_valid() {
        let state = EngineSyncState::default();
        // Valid ordering: finalized(1) <= safe(2) <= local_safe(3) <= cross_unsafe(4) <= unsafe(5)
        let update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(5)),
            cross_unsafe_head: Some(block_info(4)),
            local_safe_head: Some(block_info(3)),
            safe_head: Some(block_info(2)),
            finalized_head: Some(block_info(1)),
        };
        let result = state.apply_update(update);
        assert!(result.is_ok());
        let new_state = result.unwrap();
        assert_eq!(new_state.unsafe_head().block_info.number, 5);
        assert_eq!(new_state.cross_unsafe_head().block_info.number, 4);
        assert_eq!(new_state.local_safe_head().block_info.number, 3);
        assert_eq!(new_state.safe_head().block_info.number, 2);
        assert_eq!(new_state.finalized_head().block_info.number, 1);
    }

    #[test]
    fn test_apply_update_finalized_ahead_of_unsafe() {
        let state = EngineSyncState::default();
        // Invalid: finalized(10) > unsafe(5)
        let update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(5)),
            cross_unsafe_head: Some(block_info(5)),
            local_safe_head: Some(block_info(5)),
            safe_head: Some(block_info(5)),
            finalized_head: Some(block_info(10)),
        };
        let result = state.apply_update(update);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.finalized, 10);
        assert_eq!(err.unsafe_head, 5);
    }

    #[test]
    fn test_apply_update_safe_ahead_of_local_safe() {
        let state = EngineSyncState::default();
        // Invalid: safe(5) > local_safe(3)
        let update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(10)),
            cross_unsafe_head: Some(block_info(8)),
            local_safe_head: Some(block_info(3)),
            safe_head: Some(block_info(5)),
            finalized_head: Some(block_info(1)),
        };
        let result = state.apply_update(update);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.safe, 5);
        assert_eq!(err.local_safe, 3);
    }

    #[test]
    fn test_apply_update_cross_unsafe_ahead_of_unsafe() {
        let state = EngineSyncState::default();
        // Invalid: cross_unsafe(15) > unsafe(10)
        let update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(10)),
            cross_unsafe_head: Some(block_info(15)),
            local_safe_head: Some(block_info(5)),
            safe_head: Some(block_info(3)),
            finalized_head: Some(block_info(1)),
        };
        let result = state.apply_update(update);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.cross_unsafe, 15);
        assert_eq!(err.unsafe_head, 10);
    }

    #[test]
    fn test_apply_update_equal_heads_valid() {
        let state = EngineSyncState::default();
        // All equal is valid (edge case)
        let update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(10)),
            cross_unsafe_head: Some(block_info(10)),
            local_safe_head: Some(block_info(10)),
            safe_head: Some(block_info(10)),
            finalized_head: Some(block_info(10)),
        };
        let result = state.apply_update(update);
        assert!(result.is_ok());
    }

    #[test]
    fn test_apply_update_error_contains_previous_state() {
        // Start with a valid state
        let initial_update = EngineSyncStateUpdate {
            unsafe_head: Some(block_info(10)),
            cross_unsafe_head: Some(block_info(8)),
            local_safe_head: Some(block_info(6)),
            safe_head: Some(block_info(4)),
            finalized_head: Some(block_info(2)),
        };
        let state = EngineSyncState::default().apply_update(initial_update).unwrap();

        // Try an invalid update
        let invalid_update = EngineSyncStateUpdate {
            finalized_head: Some(block_info(100)), // Invalid: way ahead of unsafe
            ..Default::default()
        };
        let result = state.apply_update(invalid_update);
        assert!(result.is_err());
        let err = result.unwrap_err();

        // Verify error contains previous state
        assert!(err.previous_state.is_some());
        let prev = err.previous_state.unwrap();
        assert_eq!(prev.unsafe_head().block_info.number, 10);
        assert_eq!(prev.finalized_head().block_info.number, 2);

        // Verify error contains the attempted update
        assert!(err.update.finalized_head.is_some());
        assert_eq!(err.update.finalized_head.unwrap().block_info.number, 100);
    }
}
