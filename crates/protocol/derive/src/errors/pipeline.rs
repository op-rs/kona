//! This module contains derivation errors thrown within the pipeline.

use crate::errors::BuilderError;
use alloc::string::String;
use alloy_primitives::B256;
use maili_genesis::SystemConfigUpdateError;
use maili_protocol::{DepositError, SpanBatchError};
use thiserror::Error;

/// [crate::ensure] is a short-hand for bubbling up errors in the case of a condition not being met.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            return Err($err);
        }
    };
}

/// A top level filter for [PipelineError] that sorts by severity.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum PipelineErrorKind {
    /// A temporary error.
    #[error("Temporary error: {0}")]
    Temporary(#[source] PipelineError),
    /// A critical error.
    #[error("Critical error: {0}")]
    Critical(#[source] PipelineError),
    /// A reset error.
    #[error("Pipeline reset: {0}")]
    Reset(#[from] ResetError),
}

/// An error encountered during the processing.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum PipelineError {
    /// There is no data to read from the channel bank.
    #[error("EOF")]
    Eof,
    /// There is not enough data to complete the processing of the stage. If the operation is
    /// re-tried, more data will come in allowing the pipeline to progress, or eventually a
    /// [PipelineError::Eof] will be encountered.
    #[error("Not enough data")]
    NotEnoughData,
    /// No channels are available in the [ChannelProvider].
    ///
    /// [ChannelProvider]: crate::stages::ChannelProvider
    #[error("The channel provider is empty")]
    ChannelProviderEmpty,
    /// The channel has already been built by the [ChannelAssembler] stage.
    ///
    /// [ChannelAssembler]: crate::stages::ChannelAssembler
    #[error("Channel already built")]
    ChannelAlreadyBuilt,
    /// Failed to find channel in the [ChannelProvider].
    ///
    /// [ChannelProvider]: crate::stages::ChannelProvider
    #[error("Channel not found in channel provider")]
    ChannelNotFound,
    /// No channel returned by the [ChannelReader] stage.
    ///
    /// [ChannelReader]: crate::stages::ChannelReader
    #[error("The channel reader has no channel available")]
    ChannelReaderEmpty,
    /// The [BatchQueue] is empty.
    ///
    /// [BatchQueue]: crate::stages::BatchQueue
    #[error("The batch queue has no batches available")]
    BatchQueueEmpty,
    /// Missing L1 origin.
    #[error("Missing L1 origin from previous stage")]
    MissingOrigin,
    /// Missing data from [L1Retrieval].
    ///
    /// [L1Retrieval]: crate::stages::L1Retrieval
    #[error("L1 Retrieval missing data")]
    MissingL1Data,
    /// Invalid batch type passed.
    #[error("Invalid batch type passed to stage")]
    InvalidBatchType,
    /// Invalid batch validity variant.
    #[error("Invalid batch validity")]
    InvalidBatchValidity,
    /// [SystemConfig] update error.
    ///
    /// [SystemConfig]: maili_genesis::SystemConfig
    #[error("Error updating system config: {0}")]
    SystemConfigUpdate(SystemConfigUpdateError),
    /// Attributes builder error variant, with [BuilderError].
    #[error("Attributes builder error: {0}")]
    AttributesBuilder(#[from] BuilderError),
    /// [PipelineEncodingError] variant.
    #[error("Decode error: {0}")]
    BadEncoding(#[from] PipelineEncodingError),
    /// The data source can no longer provide any more data.
    #[error("Data source exhausted")]
    EndOfSource,
    /// Provider error variant.
    #[error("Blob provider error: {0}")]
    Provider(String),
}

impl PipelineError {
    /// Wrap [PipelineError] as a [PipelineErrorKind::Critical].
    pub const fn crit(self) -> PipelineErrorKind {
        PipelineErrorKind::Critical(self)
    }

    /// Wrap [PipelineError] as a [PipelineErrorKind::Temporary].
    pub const fn temp(self) -> PipelineErrorKind {
        PipelineErrorKind::Temporary(self)
    }
}

/// A reset error
#[derive(Error, Clone, Debug, Eq, PartialEq)]
pub enum ResetError {
    /// The batch has a bad parent hash.
    /// The first argument is the expected parent hash, and the second argument is the actual
    /// parent hash.
    #[error("Bad parent hash: expected {0}, got {1}")]
    BadParentHash(B256, B256),
    /// The batch has a bad timestamp.
    /// The first argument is the expected timestamp, and the second argument is the actual
    /// timestamp.
    #[error("Bad timestamp: expected {0}, got {1}")]
    BadTimestamp(u64, u64),
    /// L1 origin mismatch.
    #[error("L1 origin mismatch. Expected {0:?}, got {1:?}")]
    L1OriginMismatch(u64, u64),
    /// The stage detected a block reorg.
    /// The first argument is the expected block hash.
    /// The second argument is the parent_hash of the next l1 origin block.
    #[error("L1 reorg detected: expected {0}, got {1}")]
    ReorgDetected(B256, B256),
    /// Attributes builder error variant, with [BuilderError].
    #[error("Attributes builder error: {0}")]
    AttributesBuilder(#[from] BuilderError),
    /// A Holocene activation temporary error.
    #[error("Holocene activation reset")]
    HoloceneActivation,
}

impl ResetError {
    /// Wrap [ResetError] as a [PipelineErrorKind::Reset].
    pub const fn reset(self) -> PipelineErrorKind {
        PipelineErrorKind::Reset(self)
    }
}

/// A decoding error.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum PipelineEncodingError {
    /// The buffer is empty.
    #[error("Empty buffer")]
    EmptyBuffer,
    /// Deposit decoding error.
    #[error("Error decoding deposit: {0}")]
    DepositError(#[from] DepositError),
    /// Alloy RLP Encoding Error.
    #[error("RLP error: {0}")]
    AlloyRlpError(alloy_rlp::Error),
    /// Span Batch Error.
    #[error("{0}")]
    SpanBatchError(#[from] SpanBatchError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::error::Error;

    #[test]
    fn test_pipeline_error_kind_source() {
        let err = PipelineErrorKind::Temporary(PipelineError::Eof);
        assert!(err.source().is_some());

        let err = PipelineErrorKind::Critical(PipelineError::Eof);
        assert!(err.source().is_some());

        let err = PipelineErrorKind::Reset(ResetError::BadParentHash(
            Default::default(),
            Default::default(),
        ));
        assert!(err.source().is_some());
    }

    #[test]
    fn test_pipeline_error_source() {
        let err = PipelineError::AttributesBuilder(BuilderError::BlockMismatch(
            Default::default(),
            Default::default(),
        ));
        assert!(err.source().is_some());

        let encoding_err = PipelineEncodingError::EmptyBuffer;
        let err: PipelineError = encoding_err.into();
        assert!(err.source().is_some());

        let err = PipelineError::Eof;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_pipeline_encoding_error_source() {
        let err = PipelineEncodingError::DepositError(DepositError::UnexpectedTopicsLen(0));
        assert!(err.source().is_some());

        let err = SpanBatchError::TooBigSpanBatchSize;
        let err: PipelineEncodingError = err.into();
        assert!(err.source().is_some());

        let err = PipelineEncodingError::EmptyBuffer;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_reset_error_kinds() {
        let reset_errors = [
            ResetError::BadParentHash(Default::default(), Default::default()),
            ResetError::BadTimestamp(0, 0),
            ResetError::L1OriginMismatch(0, 0),
            ResetError::ReorgDetected(Default::default(), Default::default()),
            ResetError::AttributesBuilder(BuilderError::BlockMismatch(
                Default::default(),
                Default::default(),
            )),
            ResetError::HoloceneActivation,
        ];
        for error in reset_errors.into_iter() {
            let expected = PipelineErrorKind::Reset(error.clone());
            assert_eq!(error.reset(), expected);
        }
    }
}
