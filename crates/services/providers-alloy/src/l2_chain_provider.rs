//! Providers that use alloy provider types on the backend.

use alloy_primitives::{Bytes, U64};
use alloy_provider::{Provider, RootProvider};
use alloy_rlp::Decodable;
use alloy_transport::{RpcError, TransportErrorKind};
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind},
    traits::L2ChainProvider,
};
use maili_genesis::{RollupConfig, SystemConfig};
use maili_protocol::{to_system_config, BatchValidationProvider, L2BlockInfo};
use op_alloy_consensus::OpBlock;
use std::sync::Arc;

/// The [AlloyL2ChainProvider] is a concrete implementation of the [L2ChainProvider] trait,
/// providing data over Ethereum JSON-RPC using an alloy provider as the backend.
///
/// **Note**:
/// This provider fetches data using the `debug_getRawBlock` method. The RPC must support this
/// namespace.
#[derive(Debug, Clone)]
pub struct AlloyL2ChainProvider {
    /// The inner Ethereum JSON-RPC provider.
    inner: RootProvider,
    /// The rollup configuration.
    rollup_config: Arc<RollupConfig>,
}

impl AlloyL2ChainProvider {
    /// Creates a new [AlloyL2ChainProvider] with the given alloy provider and [RollupConfig].
    pub fn new(inner: RootProvider, rollup_config: Arc<RollupConfig>) -> Self {
        Self { inner, rollup_config }
    }

    /// Returns the chain ID.
    pub async fn chain_id(&mut self) -> Result<u64, RpcError<TransportErrorKind>> {
        self.inner.get_chain_id().await
    }

    /// Returns the latest L2 block number.
    pub async fn latest_block_number(&mut self) -> Result<u64, RpcError<TransportErrorKind>> {
        self.inner.get_block_number().await
    }

    /// Creates a new [AlloyL2ChainProvider] from the provided [reqwest::Url].
    pub fn new_http(url: reqwest::Url, rollup_config: Arc<RollupConfig>) -> Self {
        let inner = RootProvider::new_http(url);
        Self::new(inner, rollup_config)
    }
}

/// An error for the [AlloyL2ChainProvider].
#[derive(Debug, thiserror::Error)]
pub enum AlloyL2ChainProviderError {
    /// Failed to find a block.
    #[error("Failed to fetch block {0}")]
    BlockNotFound(u64),
    /// Failed to construct [L2BlockInfo] from the block and genesis.
    #[error("Failed to construct L2BlockInfo from block {0} and genesis")]
    L2BlockInfoConstruction(u64),
    /// Failed to decode an [OpBlock] from the raw block.
    #[error("Failed to decode OpBlock from raw block {0}")]
    OpBlockDecode(u64),
    /// Failed to convert the block into a [SystemConfig].
    #[error("Failed to convert block {0} into SystemConfig")]
    SystemConfigConversion(u64),
}

impl From<AlloyL2ChainProviderError> for PipelineErrorKind {
    fn from(e: AlloyL2ChainProviderError) -> Self {
        match e {
            AlloyL2ChainProviderError::BlockNotFound(_) => {
                PipelineErrorKind::Temporary(PipelineError::Provider("block not found".to_string()))
            }
            AlloyL2ChainProviderError::L2BlockInfoConstruction(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("l2 block info construction failed".to_string()),
            ),
            AlloyL2ChainProviderError::OpBlockDecode(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("op block decode failed".to_string()),
            ),
            AlloyL2ChainProviderError::SystemConfigConversion(_) => PipelineErrorKind::Temporary(
                PipelineError::Provider("system config conversion failed".to_string()),
            ),
        }
    }
}

#[async_trait]
impl BatchValidationProvider for AlloyL2ChainProvider {
    type Error = AlloyL2ChainProviderError;

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        let block = self
            .block_by_number(number)
            .await
            .map_err(|_| AlloyL2ChainProviderError::BlockNotFound(number))?;
        L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
            .map_err(|_| AlloyL2ChainProviderError::L2BlockInfoConstruction(number))
    }

    async fn block_by_number(&mut self, number: u64) -> Result<OpBlock, Self::Error> {
        let raw_block: Bytes = self
            .inner
            .raw_request("debug_getRawBlock".into(), [U64::from(number)])
            .await
            .map_err(|_| AlloyL2ChainProviderError::BlockNotFound(number))?;
        OpBlock::decode(&mut raw_block.as_ref())
            .map_err(|_| AlloyL2ChainProviderError::OpBlockDecode(number))
    }
}

#[async_trait]
impl L2ChainProvider for AlloyL2ChainProvider {
    type Error = AlloyL2ChainProviderError;

    async fn system_config_by_number(
        &mut self,
        number: u64,
        rollup_config: Arc<RollupConfig>,
    ) -> Result<SystemConfig, <Self as BatchValidationProvider>::Error> {
        let block = self
            .block_by_number(number)
            .await
            .map_err(|_| AlloyL2ChainProviderError::BlockNotFound(number))?;
        to_system_config(&block, &rollup_config)
            .map_err(|_| AlloyL2ChainProviderError::SystemConfigConversion(number))
    }
}
