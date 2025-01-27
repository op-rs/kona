//! Contains a concrete implementation of the [KeyValueStore] trait that stores data on disk,
//! using the [InteropHostCli] config.

use super::InteropHostCli;
use crate::KeyValueStore;
use alloy_primitives::{keccak256, B256};
use anyhow::Result;
use kona_preimage::PreimageKey;
use kona_proof_interop::boot::{
    L1_HEAD_KEY, L2_AGREED_PRE_STATE_KEY, L2_CHAIN_ID_KEY, L2_CLAIMED_POST_STATE_KEY,
    L2_CLAIMED_TIMESTAMP_KEY, L2_ROLLUP_CONFIG_KEY,
};

/// The default chain ID to use if none is provided.
pub(crate) const DEFAULT_CHAIN_ID: u64 = 0xbeef_babe;

/// A simple, synchronous key-value store that returns data from a [InteropHostCli] config.
#[derive(Debug)]
pub struct LocalKeyValueStore {
    cfg: InteropHostCli,
}

impl LocalKeyValueStore {
    /// Create a new [LocalKeyValueStore] with the given [InteropHostCli] config.
    pub const fn new(cfg: InteropHostCli) -> Self {
        Self { cfg }
    }
}

impl KeyValueStore for LocalKeyValueStore {
    fn get(&self, key: B256) -> Option<Vec<u8>> {
        let preimage_key = PreimageKey::try_from(*key).ok()?;
        match preimage_key.key_value() {
            L1_HEAD_KEY => Some(self.cfg.l1_head.to_vec()),
            L2_AGREED_PRE_STATE_KEY => {
                Some(keccak256(self.cfg.agreed_l2_pre_state.as_ref()).to_vec())
            }
            L2_CLAIMED_POST_STATE_KEY => Some(self.cfg.claimed_l2_post_state.to_vec()),
            L2_CLAIMED_TIMESTAMP_KEY => Some(self.cfg.claimed_l2_timestamp.to_be_bytes().to_vec()),
            L2_CHAIN_ID_KEY => Some(self.cfg.active_l2_chain_id().ok()?.to_be_bytes().to_vec()),
            L2_ROLLUP_CONFIG_KEY => {
                let rollup_configs = self.cfg.read_rollup_configs().ok()?;
                let active_rollup_config = rollup_configs
                    .get(&self.cfg.active_l2_chain_id().ok()?)
                    .cloned()
                    .unwrap_or_default();
                let serialized = serde_json::to_vec(&active_rollup_config).ok()?;
                Some(serialized)
            }
            _ => None,
        }
    }

    fn set(&mut self, _: B256, _: Vec<u8>) -> Result<()> {
        unreachable!("LocalKeyValueStore is read-only")
    }
}
