//! Errors for the `kona-preimage` crate.

use alloc::string::String;
use thiserror::Error;

/// A [PreimageOracleError] is an enum that differentiates pipe-related errors from other errors
/// in the [PreimageOracleServer] and [HintReaderServer] implementations.
///
/// [PreimageOracleServer]: crate::PreimageOracleServer
/// [HintReaderServer]: crate::HintReaderServer
#[derive(Error, Debug)]
pub enum PreimageOracleError {
    /// The pipe has been broken.
    #[error(transparent)]
    IOError(#[from] ChannelError),
    /// The preimage key is invalid.
    #[error("Invalid preimage key.")]
    InvalidPreimageKey,
    /// Key not found.
    #[error("Key not found.")]
    KeyNotFound,
    /// Buffer length mismatch.
    #[error("Buffer length mismatch. Expected {0}, got {1}.")]
    BufferLengthMismatch(usize, usize),
    /// Other errors.
    #[error("Error in preimage server: {0}")]
    Other(String),
}

/// A [Result] type for the [PreimageOracleError] enum.
pub type PreimageOracleResult<T> = Result<T, PreimageOracleError>;

/// A [ChannelError] is an enum that describes the error cases of a [Channel] trait implementation.
///
/// [Channel]: crate::Channel
#[derive(Error, Debug)]
pub enum ChannelError {
    /// The channel is closed.
    #[error("Channel is closed.")]
    Closed,
    /// Unexpected EOF.
    #[error("Unexpected EOF in channel read operation.")]
    UnexpectedEOF,
}

/// A [Result] type for the [ChannelError] enum.
pub type ChannelResult<T> = Result<T, ChannelError>;
