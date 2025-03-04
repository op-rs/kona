//! Contains derived types for interop.

use alloy_eips::eip1898::BlockNumHash;

/// A derived ID pair is a pair of block IDs, where Derived (L2) is derived from DerivedFrom (L1).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DerivedIdPair {
    /// The block ID of the L1 block.
    pub derived_from: BlockNumHash,
    /// The block ID of the L2 block.
    pub derived: BlockNumHash,
}
