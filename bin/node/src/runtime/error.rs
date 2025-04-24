//! Runtime loader error type.

use alloy_transport::{RpcError, TransportErrorKind};
use kona_rpc::ProtocolVersionError;

/// Error type for the runtime loader.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeLoaderError {
    /// Transport error
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
    /// An error resulting from decoding the protocol version
    #[error("Failed to decode protocol version: {0}")]
    ProtocolVersionDecode(#[from] ProtocolVersionError),
    /// An error occured from the [`kona_providers_alloy::AlloyChainProvider`].
    #[error(transparent)]
    ChainProvider(#[from] kona_providers_alloy::AlloyChainProviderError),
    /// Failed to convert the address slot bytes to an address
    #[error("Failed to convert address slot bytes to address: {0}")]
    AddressConversion(alloy_primitives::U256),
}
