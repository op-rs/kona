//! Errors that can occur from engine operations.

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
}
