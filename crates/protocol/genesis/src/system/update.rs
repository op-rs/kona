//! Contains the [`SystemConfigUpdate`].

use crate::{
    BatcherUpdate, Eip1559Update, GasConfigUpdate, GasLimitUpdate, OperatorFeeUpdate, SystemConfig,
    SystemConfigUpdateKind,
};

/// The system config update is an update
/// of type [`SystemConfigUpdateKind`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SystemConfigUpdate {
    /// The batcher update.
    Batcher(BatcherUpdate),
    /// The gas config update.
    GasConfig(GasConfigUpdate),
    /// The gas limit update.
    GasLimit(GasLimitUpdate),
    /// The unsafe block signer update.
    UnsafeBlockSigner,
    /// The EIP-1559 parameters update.
    Eip1559(Eip1559Update),
    /// The operator fee parameter update.
    OperatorFee(OperatorFeeUpdate),
}

impl SystemConfigUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        match self {
            Self::Batcher(update) => update.apply(config),
            Self::GasConfig(update) => update.apply(config),
            Self::GasLimit(update) => update.apply(config),
            Self::UnsafeBlockSigner => { /* Ignored in derivation */ }
            Self::Eip1559(update) => update.apply(config),
            Self::OperatorFee(update) => update.apply(config),
        }
    }

    /// Returns the update kind.
    pub const fn kind(&self) -> SystemConfigUpdateKind {
        match self {
            Self::Batcher(_) => SystemConfigUpdateKind::Batcher,
            Self::GasConfig(_) => SystemConfigUpdateKind::GasConfig,
            Self::GasLimit(_) => SystemConfigUpdateKind::GasLimit,
            Self::UnsafeBlockSigner => SystemConfigUpdateKind::UnsafeBlockSigner,
            Self::Eip1559(_) => SystemConfigUpdateKind::Eip1559,
            Self::OperatorFee(_) => SystemConfigUpdateKind::OperatorFee,
        }
    }
}
