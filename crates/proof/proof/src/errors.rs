//! Error types for the proof program.

use alloc::string::{String, ToString};
use kona_derive::errors::{PipelineError, PipelineErrorKind};
use kona_mpt::OrderedListWalkerError;
use kona_preimage::errors::PreimageOracleError;
use maili_protocol::{FromBlockError, OpBlockConversionError};
use thiserror::Error;

/// Error from an oracle-backed provider.
#[derive(Error, Debug)]
pub enum OracleProviderError {
    /// Requested block number is past the chain head.
    #[error("Block number ({0}) past chain head ({_1})")]
    BlockNumberPastHead(u64, u64),
    /// Preimage oracle error.
    #[error("Preimage oracle error: {0}")]
    Preimage(#[from] PreimageOracleError),
    /// List walker error.
    #[error("Trie walker error: {0}")]
    TrieWalker(OrderedListWalkerError),
    /// BlockInfo error.
    #[error("From block error: {0}")]
    BlockInfo(FromBlockError),
    /// Op Block conversion error.
    #[error("Op block conversion error: {0}")]
    OpBlockConversion(OpBlockConversionError),
    /// Error decoding or encoding RLP.
    #[error("RLP error: {0}")]
    Rlp(alloy_rlp::Error),
    /// Slice conversion error.
    #[error("Slice conversion error: {0}")]
    SliceConversion(core::array::TryFromSliceError),
    /// Serde error.
    #[error("Serde error: {0}")]
    Serde(serde_json::Error),
    /// Unknown Chain ID
    #[error("Unknown chain ID: {0}")]
    UnknownChainId(u64),
}

impl From<OracleProviderError> for PipelineErrorKind {
    fn from(val: OracleProviderError) -> Self {
        match val {
            OracleProviderError::BlockNumberPastHead(_, _) => PipelineError::EndOfSource.crit(),
            _ => PipelineError::Provider(val.to_string()).crit(),
        }
    }
}

/// Error parsing a hint.
#[derive(Error, Debug)]
#[error("Hint parsing error: {_0}")]
pub struct HintParsingError(pub String);
