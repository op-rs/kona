use alloy_primitives::U64;
use kona_genesis::ChainGenesis;
use kona_supervisor_types::BlockSeal;
use std::collections::HashMap;

/// Genesis provides the genesis information relevant for Interop.
#[derive(Debug, Clone)]
pub struct Genesis {
    /// The L1 [`BlockSeal`] that the rollup starts after.
    pub l1: BlockSeal,
    /// The L2 [`BlockSeal`] that the rollup starts from.
    pub l2: BlockSeal,
}

impl Genesis {
    /// Creates a new Genesis with the given L1 and L2 block seals.
    pub const fn new(l1: BlockSeal, l2: BlockSeal) -> Self {
        Self { l1, l2 }
    }

    /// Creates a new Genesis from a RollupConfig.
    pub fn new_from_rollup_genesis(genesis: ChainGenesis, l1_time: u64) -> Self {
        Self {
            l1: BlockSeal::new(genesis.l1.hash, U64::from(genesis.l1.number), U64::from(l1_time)),
            l2: BlockSeal::new(
                genesis.l2.hash,
                U64::from(genesis.l2.number),
                U64::from(genesis.l2_time),
            ),
        }
    }
}

/// RollupConfig contains the configuration for the Optimism rollup.
#[derive(Debug, Clone)]
pub struct RollupConfig {
    /// Genesis anchor information for the rollup.
    pub genesis: Genesis,

    /// The block time of the L2, in seconds.
    pub block_time: u64,

    /// Activation time for the interop network upgrade.
    pub interop_time: Option<u64>,
}

impl RollupConfig {
    /// Creates a new RollupConfig with the given genesis and block time.
    pub const fn new(genesis: Genesis, block_time: u64, interop_time: Option<u64>) -> Self {
        Self { genesis, block_time, interop_time }
    }

    /// Creates a new [`RollupConfig`] with the given genesis and block time.
    pub fn new_from_rollup_config(config: kona_genesis::RollupConfig, l1_time: u64) -> Self {
        Self {
            genesis: Genesis::new_from_rollup_genesis(config.genesis, l1_time),
            block_time: config.block_time,
            interop_time: config.hardforks.interop_time,
        }
    }
}

/// RollupConfigSet contains the configuration for multiple Optimism rollups.
#[derive(Debug, Clone, Default)]
pub struct RollupConfigSet {
    /// The rollup configurations for the Optimism rollups.
    pub rollups: HashMap<u64, RollupConfig>,
}

impl RollupConfigSet {
    /// Creates a new RollupConfigSet with the given rollup configurations.
    pub const fn new(rollups: HashMap<u64, RollupConfig>) -> Self {
        Self { rollups }
    }

    /// Returns the rollup configuration for the given chain id.
    pub fn get(&self, chain_id: u64) -> Option<&RollupConfig> {
        self.rollups.get(&chain_id)
    }

    /// adds a new rollup configuration to the set using the provided chain ID and RollupConfig.
    pub fn add_from_rollup_config(
        &mut self,
        chain_id: u64,
        config: kona_genesis::RollupConfig,
        l1_time: u64,
    ) {
        let rollup_config = RollupConfig::new_from_rollup_config(config, l1_time);
        self.rollups.insert(chain_id, rollup_config);
    }
}
