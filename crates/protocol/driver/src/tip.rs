//! Contains the tip for the derivation driver.

use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use maili_protocol::L2BlockInfo;

/// A cursor that keeps track of the L2 tip block.
#[derive(Debug, Clone)]
pub struct TipCursor {
    /// The current L2 safe head.
    pub l2_safe_head: L2BlockInfo,
    /// The header of the L2 safe head.
    pub l2_safe_head_header: Sealed<Header>,
    /// The output root of the L2 safe head.
    pub l2_safe_head_output_root: B256,
}

impl TipCursor {
    /// Instantiates a new `SyncCursor`.
    pub const fn new(
        l2_safe_head: L2BlockInfo,
        l2_safe_head_header: Sealed<Header>,
        l2_safe_head_output_root: B256,
    ) -> Self {
        Self { l2_safe_head, l2_safe_head_header, l2_safe_head_output_root }
    }

    /// Returns the current L2 safe head.
    pub const fn l2_safe_head(&self) -> &L2BlockInfo {
        &self.l2_safe_head
    }

    /// Returns the header of the L2 safe head.
    pub const fn l2_safe_head_header(&self) -> &Sealed<Header> {
        &self.l2_safe_head_header
    }

    /// Returns the output root of the L2 safe head.
    pub const fn l2_safe_head_output_root(&self) -> &B256 {
        &self.l2_safe_head_output_root
    }
}
