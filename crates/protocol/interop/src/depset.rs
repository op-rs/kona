use alloy_primitives::ChainId;
use kona_registry::HashMap;
use crate::MESSAGE_EXPIRY_WINDOW;

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

impl DependencySet {
  /// Checks if a message is eligible for execution at the given timestamp.
  ///
  /// Returns an error if the system cannot currently determine eligibility,
  /// for example, while the `DependencySet` is still syncing.
  pub fn can_execute_at(&self, chain_id: ChainId, exec_timestamp: u64) -> bool {
    self.dependencies.get(&chain_id).map_or(false, |dependency| {
      exec_timestamp >= dependency.activation_time
    })
  }

  /// Determines whether an initiating message can be pulled in at the given timestamp.
  ///
  /// Returns an error if the message from the specified chain is not currently readable,
  /// for example, if the `DependencySet` is still syncing new data.
  pub fn can_initiate_at(&self, chain_id: ChainId, init_timestamp: u64) -> bool {
    self.dependencies.get(&chain_id).map_or(false, |dependency| {
      init_timestamp >= dependency.history_min_time
    })
  }

  /// Returns the message expiry window associated with this dependency set.
  pub fn get_message_expiry_window(&self) -> u64 {
    if self.override_message_expiry_window == 0 {
      return MESSAGE_EXPIRY_WINDOW;
    }
    return self.override_message_expiry_window;
  }
}