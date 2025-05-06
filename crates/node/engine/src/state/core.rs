//! The internal state of the engine controller.

use crate::SyncStatus;
use alloy_rpc_types_engine::ForkchoiceState;
use kona_protocol::L2BlockInfo;

#[cfg(feature = "metrics")]
use crate::metrics::ENGINE_UNSAFE_HEAD_HEIGHT_NAME;

/// The chain state viewed by the engine controller.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EngineState {
    /// Most recent block found on the p2p network
    pub(crate) unsafe_head: L2BlockInfo,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    pub(crate) cross_unsafe_head: L2BlockInfo,
    /// Pending localSafeHead
    /// L2 block processed from the middle of a span batch,
    /// but not marked as the safe block yet.
    pub(crate) pending_safe_head: L2BlockInfo,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    pub(crate) local_safe_head: L2BlockInfo,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    pub(crate) safe_head: L2BlockInfo,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    pub(crate) finalized_head: L2BlockInfo,
    /// The unsafe head to roll back to,
    /// after the pending safe head fails to become safe.
    /// This is changing in the Holocene fork.
    pub(crate) backup_unsafe_head: Option<L2BlockInfo>,

    /// The [SyncStatus] of the engine.
    pub sync_status: SyncStatus,

    /// If a forkchoice update call is needed.
    pub forkchoice_update_needed: bool,

    /// Track when the rollup node changes the forkchoice to restore previous
    /// known unsafe chain. e.g. Unsafe Reorg caused by Invalid span batch.
    /// This update does not retry except engine returns non-input error
    /// because engine may forgot backupUnsafeHead or backupUnsafeHead is not part
    /// of the chain.
    pub need_fcu_call_backup_unsafe_reorg: bool,
}

impl EngineState {
    /// Creates a `ForkchoiceState`
    ///
    /// - `head_block` = `unsafe_head`
    /// - `safe_block` = `safe_head`
    /// - `finalized_block` = `finalized_head`
    ///
    /// If the block info is not yet available, the default values are used.
    pub const fn create_forkchoice_state(&self) -> ForkchoiceState {
        ForkchoiceState {
            head_block_hash: self.unsafe_head.block_info.hash,
            safe_block_hash: self.safe_head.block_info.hash,
            finalized_block_hash: self.finalized_head.block_info.hash,
        }
    }

    /// Returns if consolidation is needed.
    ///
    /// [Consolidation] is only performed by a rollup node when the unsafe head
    /// is ahead of the safe head. When the two are equal, consolidation isn't
    /// required and the [`crate::BuildTask`] can be used to build the block.
    ///
    /// [Consolidation]: https://specs.optimism.io/protocol/derivation.html#l1-consolidation-payload-attributes-matching
    pub fn needs_consolidation(&self) -> bool {
        self.safe_head() != self.unsafe_head()
    }

    /// Returns the current unsafe head.
    pub const fn unsafe_head(&self) -> L2BlockInfo {
        self.unsafe_head
    }

    /// Returns the current cross-verified unsafe head.
    pub const fn cross_unsafe_head(&self) -> L2BlockInfo {
        self.cross_unsafe_head
    }

    /// Returns the current pending safe head.
    pub const fn pending_safe_head(&self) -> L2BlockInfo {
        self.pending_safe_head
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

    /// Returns the current backup unsafe head.
    pub const fn backup_unsafe_head(&self) -> Option<L2BlockInfo> {
        self.backup_unsafe_head
    }

    /// Set the unsafe head.
    pub fn set_unsafe_head(&mut self, unsafe_head: L2BlockInfo) {
        #[cfg(feature = "metrics")]
        {
            let height = unsafe_head.block_info.number;
            set!(gauge, ENGINE_UNSAFE_HEAD_HEIGHT_NAME, height as f64);
        }
        self.unsafe_head = unsafe_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the cross-verified unsafe head.
    pub fn set_cross_unsafe_head(&mut self, cross_unsafe_head: L2BlockInfo) {
        self.cross_unsafe_head = cross_unsafe_head;
    }

    /// Set the pending safe head.
    pub fn set_pending_safe_head(&mut self, pending_safe_head: L2BlockInfo) {
        self.pending_safe_head = pending_safe_head;
    }

    /// Set the local safe head.
    pub fn set_local_safe_head(&mut self, local_safe_head: L2BlockInfo) {
        self.local_safe_head = local_safe_head;
    }

    /// Set the safe head.
    pub fn set_safe_head(&mut self, safe_head: L2BlockInfo) {
        self.safe_head = safe_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the finalized head.
    pub fn set_finalized_head(&mut self, finalized_head: L2BlockInfo) {
        self.finalized_head = finalized_head;
        self.forkchoice_update_needed = true;
    }

    /// Set the backup unsafe head.
    pub fn set_backup_unsafe_head(&mut self, backup_unsafe_head: L2BlockInfo, reorg: bool) {
        self.backup_unsafe_head = Some(backup_unsafe_head);
        self.need_fcu_call_backup_unsafe_reorg = reorg;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncStatus;
    use alloy_eips::BlockNumHash;
    use alloy_primitives::B256;
    use kona_protocol::BlockInfo;

    #[cfg(feature = "metrics")]
    use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
    #[cfg(feature = "metrics")]
    use std::sync::Mutex;

    #[cfg(feature = "metrics")]
    static GLOBAL_TEST_PROMETHEUS_HANDLE: Mutex<Option<PrometheusHandle>> = Mutex::new(None);

    #[cfg(feature = "metrics")]
    fn init_test_recorder() -> PrometheusHandle {
        let mut guard = GLOBAL_TEST_PROMETHEUS_HANDLE.lock().unwrap_or_else(|poisoned| {
            panic!("Prometheus handle mutex was poisoned: {:?}", poisoned);
        });

        if let Some(handle) = guard.as_ref() {
            return handle.clone();
        }

        let builder = PrometheusBuilder::new();
        let handle =
            builder.install_recorder().expect("Failed to install Prometheus recorder for tests");

        *guard = Some(handle.clone());

        handle
    }

    fn dummy_b256(val: u64, differentiator: u8) -> B256 {
        let mut bytes = [0u8; 32];
        bytes[0] = differentiator;
        bytes[24..32].copy_from_slice(&val.to_be_bytes());
        B256::new(bytes)
    }

    fn l2_block_info(number: u64) -> L2BlockInfo {
        let l1_origin_block_info = BlockInfo {
            hash: dummy_b256(number.saturating_sub(1), 1),
            number: number.saturating_sub(1),
            parent_hash: dummy_b256(number.saturating_sub(2), 2),
            timestamp: 10000 + number.saturating_sub(1) * 12,
        };
        let l1_origin =
            BlockNumHash { hash: l1_origin_block_info.hash, number: l1_origin_block_info.number };

        L2BlockInfo {
            block_info: BlockInfo {
                hash: dummy_b256(number, 3),
                number,
                parent_hash: dummy_b256(number.saturating_sub(1), 4),
                timestamp: 20000 + number * 2,
            },
            l1_origin,
            seq_num: number,
        }
    }

    fn initial_engine_state() -> EngineState {
        let genesis_l2_info = l2_block_info(0);
        EngineState {
            unsafe_head: genesis_l2_info,
            cross_unsafe_head: genesis_l2_info,
            pending_safe_head: genesis_l2_info,
            local_safe_head: genesis_l2_info,
            safe_head: genesis_l2_info,
            finalized_head: genesis_l2_info,
            backup_unsafe_head: None,
            sync_status: SyncStatus::default(),
            forkchoice_update_needed: false,
            need_fcu_call_backup_unsafe_reorg: false,
        }
    }

    #[test]
    #[cfg(feature = "metrics")]
    fn test_unsafe_head_height_metric() {
        let handle = init_test_recorder();

        let mut state = initial_engine_state();

        let new_unsafe_height = 123u64;
        let new_unsafe_info = l2_block_info(new_unsafe_height);
        state.set_unsafe_head(new_unsafe_info);

        let rendered = handle.render();

        let expected_metric_line = format!(
            "{name} {value}",
            name = ENGINE_UNSAFE_HEAD_HEIGHT_NAME,
            value = new_unsafe_height as f64
        );

        assert!(
            rendered.contains(&format!("# TYPE {} gauge", ENGINE_UNSAFE_HEAD_HEIGHT_NAME)) ||
                rendered.contains(ENGINE_UNSAFE_HEAD_HEIGHT_NAME),
            "Metric name or type for '{}' not found in output:\n{}",
            ENGINE_UNSAFE_HEAD_HEIGHT_NAME,
            rendered
        );
        assert!(
            rendered.lines().any(|line| line.trim() == expected_metric_line.trim()),
            "Expected metric line '{}' not found in output:\n{}",
            expected_metric_line,
            rendered
        );

        let newer_unsafe_height = 456u64;
        let newer_unsafe_info = l2_block_info(newer_unsafe_height);
        state.set_unsafe_head(newer_unsafe_info);

        let rendered_updated = handle.render();

        let expected_updated_metric_line = format!(
            "{name} {value}",
            name = ENGINE_UNSAFE_HEAD_HEIGHT_NAME,
            value = newer_unsafe_height as f64
        );
        let previous_metric_line = expected_metric_line;

        assert!(
            rendered_updated.lines().any(|line| line.trim() == expected_updated_metric_line.trim()),
            "Expected updated metric line '{}' not found in output:\n{}",
            expected_updated_metric_line,
            rendered_updated
        );
        assert!(
            !rendered_updated.lines().any(|line| line.trim() == previous_metric_line.trim()),
            "Previous metric line '{}' should not be present in updated output:\n{}",
            previous_metric_line,
            rendered_updated
        );
    }
}
