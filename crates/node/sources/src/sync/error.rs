//! Contains the error types used for finding the starting forkchoice state.

use alloy_primitives::B256;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_providers_alloy::{AlloyChainProviderError, AlloyL2ChainProviderError};
use thiserror::Error;

/// An error that can occur during the sync start process.
#[derive(Error, Debug)]
pub enum SyncStartError {
    /// An error occurred in the L1 chain provider.
    #[error(transparent)]
    L1ChainProvider(#[from] AlloyChainProviderError),
    /// An error occurred in the L2 chain provider.
    #[error(transparent)]
    L2ChainProvider(#[from] AlloyL2ChainProviderError),
    /// An rpc error occurred
    #[error("An RPC error occurred: {0}")]
    RpcError(#[from] RpcError<TransportErrorKind>),
    /// Invalid L1 genesis hash.
    #[error("Invalid L1 genesis hash. Expected {0}, Got {1}")]
    InvalidL1GenesisHash(B256, B256),
    /// Invalid L2 genesis hash.
    #[error("Invalid L2 genesis hash. Expected {0}, Got {1}")]
    InvalidL2GenesisHash(B256, B256),
    /// Finalized block mismatch
    #[error("Finalized block mismatch. Expected {0}, Got {1}")]
    MismatchedFinalizedBlock(B256, B256),
    /// L1 origin mismatch.
    #[error("L1 origin mismatch")]
    L1OriginMismatch,
    /// Non-zero sequence number.
    #[error("Non-zero sequence number for block with different L1 origin")]
    NonZeroSequenceNumber,
    /// Inconsistent sequence number.
    #[error("Inconsistent sequence number; Must monotonically increase.")]
    InconsistentSequenceNumber,
}
