//! Contains driver-related error types.

use soon_derive::errors::PipelineErrorKind;
use thiserror::Error;

/// A [Result] type for the [DriverError].
pub type DriverResult<T, E> = Result<T, DriverError<E>>;

/// Driver error.
#[derive(Error, Debug)]
pub enum DriverError<E>
where
    E: core::error::Error,
{
    /// Pipeline error.
    #[error("Pipeline error: {0}")]
    Pipeline(#[from] PipelineErrorKind),
    /// An error returned by the executor.
    #[error("Executor error: {0}")]
    Executor(E),
    /// Error decoding or encoding RLP.
    #[error("RLP error: {0}")]
    Rlp(alloy_rlp::Error),
}
