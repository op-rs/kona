//! Contains the hardfork configuration for the chain.

/// Hardfork configuration.
///
/// See: <https://github.com/ethereum-optimism/superchain-registry/blob/8ff62ada16e14dd59d0fb94ffb47761c7fa96e01/ops/internal/config/chain.go#L102-L110>
#[derive(Debug, Copy, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HardForkConfiguration {
    /// Canyon hardfork activation time
    pub canyon_time: Option<u64>,
    /// Delta hardfork activation time
    pub delta_time: Option<u64>,
    /// Ecotone hardfork activation time
    pub ecotone_time: Option<u64>,
    /// Fjord hardfork activation time
    pub fjord_time: Option<u64>,
    /// Granite hardfork activation time
    pub granite_time: Option<u64>,
    /// Holocene hardfork activation time
    pub holocene_time: Option<u64>,
    /// Isthmus hardfork activation time
    pub isthmus_time: Option<u64>,
    /// Interop hardfork activation time
    pub interop_time: Option<u64>,
}
