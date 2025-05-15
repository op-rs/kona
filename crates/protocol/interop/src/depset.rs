use alloy_primitives::ChainId;
use kona_registry::HashMap;

/// Configuration for a dependency of a chain
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ChainDependency {
    /// chain_index is the unique short identifier for this chain
    pub chain_index: u32,

    /// activation_time is the timestamp (in seconds) when the chain becomes part of the dependency
    /// set. This is the minimum timestamp of the inclusion of an executing message that can be
    /// considered valid.
    pub activation_time: u64,

    /// history_min_time is the lower bound timestamp (in seconds) for data storage and
    /// accessibility. This is the minimum timestamp of an initiating message that is required
    /// to be accessible to other chains. This is set to 0 when all data since genesis is
    /// executable.
    pub history_min_time: u64,
}

/// Configuration for the depedency set
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DependencySet {
    /// dependencies information per chain
    pub dependencies: HashMap<ChainId, ChainDependency>,

    /// Override message expiry window to use for this dependency set.
    pub override_message_expiry_window: u64,
}
