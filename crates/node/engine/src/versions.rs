//! Provides engine api endpoint versions
//!
//! Adapted from the [op-node version providers][vp].
//!
//! [vp]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/types.go#L546

use kona_genesis::RollupConfig;

/// The method version for the `engine_forkchoiceUpdated` api.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineForkchoiceVersion {
    /// The `engine_forkchoiceUpdated` api version 1.
    V1,
    /// The `engine_forkchoiceUpdated` api version 2.
    V2,
    /// The `engine_forkchoiceUpdated` api version 3.
    V3,
}

impl EngineForkchoiceVersion {
    /// Returns the appropriate [`EngineForkchoiceVersion`] for the chain at the given attributes.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_ecotone_active(timestamp) {
            // Cancun
            Self::V3
        } else if cfg.is_canyon_active(timestamp) {
            // Shanghai
            Self::V2
        } else {
            // According to Ethereum engine API spec, we can use fcuV2 here,
            // but Geth v1.13.11 does not accept V2 before Shanghai.
            Self::V1
        }
    }
}

/// The method version for the `engine_newPayload` api.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineNewPayloadVersion {
    /// The `engine_newPayload` api version 2.
    V2,
    /// The `engine_newPayload` api version 3.
    V3,
    /// The `engine_newPayload` api version 4.
    V4,
}

impl EngineNewPayloadVersion {
    /// Returns the appropriate [`EngineNewPayloadVersion`] for the chain at the given timestamp.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_isthmus_active(timestamp) {
            Self::V4
        } else if cfg.is_ecotone_active(timestamp) {
            // Cancun
            Self::V3
        } else {
            Self::V2
        }
    }
}

/// The method version for the `engine_getPayload` api.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineGetPayloadVersion {
    /// The `engine_getPayload` api version 2.
    V2,
    /// The `engine_getPayload` api version 3.
    V3,
    /// The `engine_getPayload` api version 4.
    V4,
}

impl EngineGetPayloadVersion {
    /// Returns the appropriate [`EngineGetPayloadVersion`] for the chain at the given timestamp.
    ///
    /// Uses the [`RollupConfig`] to check which hardfork is active at the given timestamp.
    pub fn from_cfg(cfg: &RollupConfig, timestamp: u64) -> Self {
        if cfg.is_isthmus_active(timestamp) {
            Self::V4
        } else if cfg.is_ecotone_active(timestamp) {
            // Cancun
            Self::V3
        } else {
            Self::V2
        }
    }
}
