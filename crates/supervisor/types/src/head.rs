//! Head of chain in context of superchain.

use kona_protocol::BlockInfo;

/// Head of a chain from superchain perspective.
///
/// In context of a single chain, canonical head is tracked by its safe and finalized head. In
/// superchain context, earlier finality-stages (aka [`SafetyLevel`]s) are tracked too, i.e.
/// unsafe, cross-unsafe and local-safe heads.
///
/// [`SafetyLevel`]: op_alloy_consensus::interop::SafetyLevel
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuperHead {
    /// Source (L1) block.
    pub l1_source: BlockInfo,
    /// [`Unsafe`] head of chain.
    ///
    /// [`Unsafe`]: op_alloy_consensus::interop::SafetyLevel::Unsafe
    pub local_unsafe: BlockInfo,
    /// [`CrossUnsafe`] head of chain.
    ///
    /// [`CrossUnsafe`]: op_alloy_consensus::interop::SafetyLevel::CrossUnsafe
    pub cross_unsafe: BlockInfo,
    /// [`LocalSafe`] head of chain.
    ///
    /// [`LocalSafe`]: op_alloy_consensus::interop::SafetyLevel::LocalSafe
    pub local_safe: BlockInfo,
    /// [`Safe`] head of chain.
    ///
    /// [`Safe`]: op_alloy_consensus::interop::SafetyLevel::Safe
    pub cross_safe: BlockInfo,
    /// [`Finalized`] head of chain.
    ///
    /// [`Finalized`]: op_alloy_consensus::interop::SafetyLevel::Finalized
    pub finalized: BlockInfo,
}
