use crate::MESSAGE_EXPIRY_WINDOW;
use alloy_primitives::ChainId;
use kona_registry::HashMap;

/// Configuration for a dependency of a chain
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ChainDependency {
    /// The unique short identifier for this chain.
    pub chain_index: u32,

    /// Timestamp (in seconds) when the chain becomes part of the dependency set.
    /// This is the minimum timestamp of the inclusion of an executing message that can be
    /// considered valid.
    pub activation_time: u64,

    /// Lower bound timestamp (in seconds) for data storage and accessibility.
    /// This is the minimum timestamp of an initiating message that is required to be
    /// accessible to other chains. This is set to 0 when all data since genesis is executable.
    pub history_min_time: u64,
}

/// Configuration for the depedency set
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DependencySet {
    /// Dependencies information per chain.
    pub dependencies: HashMap<ChainId, ChainDependency>,

    /// Override message expiry window to use for this dependency set.
    pub override_message_expiry_window: u64,
}

impl DependencySet {
    /// Determines whether a message is eligible for execution at the given timestamp.
    ///
    /// Returns an error if the system cannot currently determine eligibility,
    /// for example, while the `DependencySet` is still syncing.
    pub fn can_execute_at(&self, chain_id: ChainId, exec_timestamp: u64) -> bool {
        self.dependencies
            .get(&chain_id)
            .is_some_and(|dependency| exec_timestamp >= dependency.activation_time)
    }

    /// Determines whether an initiating message can be pulled in at the given timestamp.
    ///
    /// Returns an error if the message from the specified chain is not currently readable,
    /// for example, if the `DependencySet` is still syncing new data.
    pub fn can_initiate_at(&self, chain_id: ChainId, init_timestamp: u64) -> bool {
        self.dependencies
            .get(&chain_id)
            .is_some_and(|dependency| init_timestamp >= dependency.history_min_time)
    }

    /// Returns the message expiry window associated with this dependency set.
    pub const fn get_message_expiry_window(&self) -> u64 {
        if self.override_message_expiry_window == 0 {
            return MESSAGE_EXPIRY_WINDOW;
        }
        self.override_message_expiry_window
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::ChainId;
    use kona_registry::HashMap;

    const fn create_dependency_set(
        dependencies: HashMap<ChainId, ChainDependency>,
        override_expiry: u64,
    ) -> DependencySet {
        DependencySet { dependencies, override_message_expiry_window: override_expiry }
    }

    #[test]
    fn test_can_execute_at_chain_exists_and_active() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(ds.can_execute_at(1, 100), "Should be able to execute at activation time");
        assert!(ds.can_execute_at(1, 150), "Should be able to execute after activation time");
    }

    #[test]
    fn test_can_execute_at_chain_exists_but_not_active() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(!ds.can_execute_at(1, 99), "Should not be able to execute before activation time");
        assert!(
            !ds.can_execute_at(1, 0),
            "Should not be able to execute much before activation time"
        );
    }

    #[test]
    fn test_can_execute_at_chain_does_not_exist() {
        let deps = HashMap::default(); // Empty dependencies
        let ds = create_dependency_set(deps, 0);

        assert!(
            !ds.can_execute_at(1, 100),
            "Should not be able to execute if chain does not exist"
        );
    }

    #[test]
    fn test_can_execute_at_multiple_chains() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        deps.insert(
            2,
            ChainDependency { chain_index: 2, activation_time: 200, history_min_time: 150 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(ds.can_execute_at(1, 100));
        assert!(!ds.can_execute_at(2, 100)); // Chain 2 not active yet
        assert!(ds.can_execute_at(2, 200));
    }

    #[test]
    fn test_can_initiate_at_chain_exists_and_after_history_min_time() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(ds.can_initiate_at(1, 50), "Should be able to initiate at history_min_time");
        assert!(ds.can_initiate_at(1, 75), "Should be able to initiate after history_min_time");
    }

    #[test]
    fn test_can_initiate_at_chain_exists_but_before_history_min_time() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(
            !ds.can_initiate_at(1, 49),
            "Should not be able to initiate before history_min_time"
        );
        assert!(
            !ds.can_initiate_at(1, 0),
            "Should not be able to initiate much before history_min_time"
        );
    }

    #[test]
    fn test_can_initiate_at_chain_does_not_exist() {
        let deps = HashMap::default(); // Empty dependencies
        let ds = create_dependency_set(deps, 0);

        assert!(
            !ds.can_initiate_at(1, 50),
            "Should not be able to initiate if chain does not exist"
        );
    }

    #[test]
    fn test_can_initiate_at_multiple_chains() {
        let mut deps = HashMap::default();
        deps.insert(
            1,
            ChainDependency { chain_index: 1, activation_time: 100, history_min_time: 50 },
        );
        deps.insert(
            2,
            ChainDependency { chain_index: 2, activation_time: 200, history_min_time: 150 },
        );
        let ds = create_dependency_set(deps, 0);

        assert!(ds.can_initiate_at(1, 50));
        assert!(ds.can_initiate_at(1, 100));
        assert!(!ds.can_initiate_at(2, 50)); // Chain 2 history not available yet
        assert!(!ds.can_initiate_at(2, 149)); // Chain 2 history not available yet
        assert!(ds.can_initiate_at(2, 150));
        assert!(ds.can_initiate_at(2, 200));
    }

    #[test]
    fn test_get_message_expiry_window_default() {
        let deps = HashMap::default();
        // override_message_expiry_window is 0, so default should be used
        let ds = create_dependency_set(deps, 0);
        assert_eq!(
            ds.get_message_expiry_window(),
            MESSAGE_EXPIRY_WINDOW,
            "Should return default expiry window when override is 0"
        );
    }

    #[test]
    fn test_get_message_expiry_window_override() {
        let deps = HashMap::default();
        let override_value = 12345;
        let ds = create_dependency_set(deps, override_value);
        assert_eq!(
            ds.get_message_expiry_window(),
            override_value,
            "Should return override expiry window when it's non-zero"
        );
    }
}
