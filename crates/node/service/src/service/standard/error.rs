//! Contains the error type for the [`crate::RollupNode`].

use jsonrpsee::server::RegisterMethodError;
use kona_derive::errors::PipelineErrorKind;
use kona_p2p::NetworkBuilderError;
use kona_providers_alloy::AlloyChainProviderError;
use kona_rpc::RpcLauncherError;
use kona_sources::SyncStartError;

/// Errors that can occur during the operation of the [`crate::RollupNode`].
#[derive(thiserror::Error, Debug)]
pub enum RollupNodeError {
    /// An error occurred while finding the sync starting point.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// An error occurred while creating the derivation pipeline.
    #[error(transparent)]
    OnlinePipeline(#[from] PipelineErrorKind),
    /// An error occurred while initializing the derivation pipeline.
    #[error(transparent)]
    AlloyChainProvider(#[from] AlloyChainProviderError),
    /// An error occured while initializing the network.
    #[error(transparent)]
    Network(#[from] NetworkBuilderError),
    /// An error occured while launching the RPC server.
    #[error(transparent)]
    RpcLauncher(#[from] RpcLauncherError),
    /// An error occurred while registering RPC methods.
    #[error(transparent)]
    RegisterMethod(#[from] RegisterMethodError),
}
