//! Errors that can occur from engine operations.

use alloc::string::String;
use alloy_transport::TransportError;
use derive_more::{Display, From};
use thiserror::Error;

/// An error occuring from an engine update.
#[derive(Debug, Display, From, Clone, Error, PartialEq, Eq)]
pub enum EngineUpdateError {
    /// No forkchoice update is needed.
    #[display("No forkchoice update is needed.")]
    NoForkchoiceUpdateNeeded,
    /// The forkchoice state is invalid.
    #[display("Invalid forkchoice state, unsafe head {_0} is behind finalized head {_1}.")]
    InvalidForkchoiceState(u64, u64),
    /// The forkchoice update failed.
    #[display("The forkchoice update failed.")]
    ForkchoiceUpdateFailed,
    /// A reset is needed.
    #[display("A reset is needed.")]
    Reset,
    /// A temporary error
    #[display("Temporary error: {_0}")]
    Temporary(String),
}

impl From<TransportError> for EngineUpdateError {
    fn from(e: TransportError) -> Self {
        match e {
            // See: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L345
            TransportError::ErrorResp(_) => Self::Reset,
            _ => Self::Temporary(e.to_string()),
        }
    }
}
