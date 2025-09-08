//! Contains the cursor for the derivation pipeline.

use alloc::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use alloy_consensus::{Header, Sealed};
use alloy_primitives::{B256, map::HashMap};
use kona_protocol::{BlockInfo, L2BlockInfo};

use crate::TipCursor;

/// A cursor that tracks the pipeline tip.
#[derive(Debug, Clone)]
pub struct PipelineCursor {
    /// The block cache capacity before evicting old entries
    /// (to avoid unbounded memory growth)
    pub capacity: usize,
    /// The channel timeout used to create the cursor.
    pub channel_timeout: u64,
    /// The l1 Origin of the pipeline.
    pub origin: BlockInfo,
    /// The L1 origin block numbers for which we have an L2 block in the cache.
    /// Used to keep track of the order of insertion and evict the oldest entry.
    pub origins: VecDeque<u64>,
    /// The L1 origin block info for which we have an L2 block in the cache.
    pub origin_infos: HashMap<u64, BlockInfo>,
    /// A map from the l1 origin block number to its L2 tip.
    pub tips: BTreeMap<u64, TipCursor>,
}

impl PipelineCursor {
    /// Create a new cursor with the default cache capacity
    pub fn new(channel_timeout: u64, origin: BlockInfo) -> Self {
        // NOTE: capacity must be greater than the `channel_timeout` to allow
        // for derivation to proceed through a deep reorg.
        // Ref: <https://specs.optimism.io/protocol/derivation.html#timeouts>
        let capacity = channel_timeout as usize + 5;

        let mut origins = VecDeque::with_capacity(capacity);
        origins.push_back(origin.number);
        let mut origin_infos = HashMap::default();
        origin_infos.insert(origin.number, origin);
        Self { capacity, channel_timeout, origin, origins, origin_infos, tips: Default::default() }
    }

    /// Returns the current origin of the pipeline.
    pub const fn origin(&self) -> BlockInfo {
        self.origin
    }

    /// Returns the current L2 safe head.
    pub fn l2_safe_head(&self) -> &L2BlockInfo {
        &self.tip().l2_safe_head
    }

    /// Returns the header of the L2 safe head.
    pub fn l2_safe_head_header(&self) -> &Sealed<Header> {
        &self.tip().l2_safe_head_header
    }

    /// Returns the output root of the L2 safe head.
    pub fn l2_safe_head_output_root(&self) -> &B256 {
        &self.tip().l2_safe_head_output_root
    }

    /// Get the current L2 tip
    pub fn tip(&self) -> &TipCursor {
        if let Some((_, l2_tip)) = self.tips.last_key_value() {
            l2_tip
        } else {
            unreachable!("cursor must be initialized with one block before advancing")
        }
    }

    /// Advance the cursor to the provided L2 block, given the corresponding L1 origin block.
    ///
    /// If the cache is full, the oldest entry is evicted.
    pub fn advance(&mut self, origin: BlockInfo, l2_tip_block: TipCursor) {
        if self.tips.len() >= self.capacity {
            let key = self.origins.pop_front().unwrap();
            self.tips.remove(&key);
        }

        self.origin = origin;
        self.origins.push_back(origin.number);
        self.origin_infos.insert(origin.number, origin);
        self.tips.insert(origin.number, l2_tip_block);
    }

    /// When the L1 undergoes a reorg, we need to reset the cursor to the fork block minus
    /// the channel timeout, because an L2 block might have started to be derived at the
    /// beginning of the channel.
    ///
    /// Returns the (L2 block info, L1 origin block info) tuple for the new cursor state.
    pub fn reset(&mut self, fork_block: u64) -> (TipCursor, BlockInfo) {
        let channel_start = fork_block - self.channel_timeout;

        match self.tips.get(&channel_start) {
            Some(l2_safe_tip) => {
                // The channel start block is in the cache, we can use it to reset the cursor.
                (l2_safe_tip.clone(), self.origin_infos[&channel_start])
            }
            None => {
                // If the channel start block is not in the cache, we reset the cursor
                // to the closest known L1 block for which we have a corresponding L2 block.
                let (last_l1_known_tip, l2_known_tip) = self
                    .tips
                    .range(..=channel_start)
                    .next_back()
                    .expect("walked back to genesis without finding anchor origin block");

                (l2_known_tip.clone(), self.origin_infos[last_l1_known_tip])
            }
        }
    }
}
