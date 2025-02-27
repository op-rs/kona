//! Contains the engine controller.
//!
//! See: <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46>

use kona_genesis::RollupConfig;
use kona_rpc::L2BlockRef;

use crate::{
    engine::{EngineClient, EngineState, SyncStatus},
    sync::SyncConfig,
};

/// The engine controller.
#[derive(Debug, Clone)]
pub struct EngineController {
    /// The internal engine client.
    pub client: EngineClient,
    /// The sync status.
    pub sync_status: SyncStatus,
    /// The engine state.
    pub state: EngineState,

    // Below are extracted fields from the `RollupConfig`.
    // Since they don't change during the lifetime of the `EngineController`,
    // we don't need to store a reference to the `RollupConfig`.
    /// Blocktime of the L2 chain
    pub blocktime: u64,
    /// The ecotone timestamp used for fork choice
    pub ecotone_timestamp: Option<u64>,
    /// The canyon timestamp used for fork choice
    pub canyon_timestamp: Option<u64>,
}

impl EngineController {
    /// Creates a new engine controller.
    pub fn new(client: EngineClient, config: &RollupConfig, sync: SyncConfig) -> Self {
        let sync_status = SyncStatus::from(sync.sync_mode);
        Self {
            client,
            sync_status,
            state: EngineState::new(L2BlockRef {
                l1_block_info: Default::default(),
                l1_origin: Default::default(),
                sequence_number: 0,
            }),
            blocktime: config.block_time,
            ecotone_timestamp: config.hardforks.ecotone_time,
            canyon_timestamp: config.hardforks.canyon_time,
        }
    }
}
