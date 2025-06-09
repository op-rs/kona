//! Providers that use alloy provider types on the backend.

use alloy_consensus::{Header, Receipt, TxEnvelope};
use alloy_eips::BlockId;
use alloy_primitives::B256;
use alloy_provider::{Provider, RootProvider};
use alloy_transport::{RpcError, TransportErrorKind};
use async_trait::async_trait;
use kona_derive::{PipelineError, PipelineErrorKind, traits::ChainProvider};
use kona_protocol::BlockInfo;
use lru::LruCache;
use std::{boxed::Box, num::NonZeroUsize, vec::Vec};

/// The [AlloyChainProvider] is a concrete implementation of the [ChainProvider] trait, providing
/// data over Ethereum JSON-RPC using an alloy provider as the backend.
#[derive(Debug, Clone)]
pub struct AlloyChainProvider {
    /// The inner Ethereum JSON-RPC provider.
    pub inner: RootProvider,
    /// `header_by_hash` LRU cache.
    header_by_hash_cache: LruCache<B256, Header>,
    /// `receipts_by_hash_cache` LRU cache.
    receipts_by_hash_cache: LruCache<B256, Vec<Receipt>>,
    /// `block_info_and_transactions_by_hash` LRU cache.
    block_info_and_transactions_by_hash_cache: LruCache<B256, (BlockInfo, Vec<TxEnvelope>)>,
}

impl AlloyChainProvider {
    /// Creates a new [AlloyChainProvider] with the given alloy provider.
    ///
    /// ## Panics
    /// - Panics if `cache_size` is zero.
    pub fn new(inner: RootProvider, cache_size: usize) -> Self {
        Self {
            inner,
            header_by_hash_cache: LruCache::new(NonZeroUsize::new(cache_size).unwrap()),
            receipts_by_hash_cache: LruCache::new(NonZeroUsize::new(cache_size).unwrap()),
            block_info_and_transactions_by_hash_cache: LruCache::new(
                NonZeroUsize::new(cache_size).unwrap(),
            ),
        }
    }

    /// Creates a new [AlloyChainProvider] from the provided [reqwest::Url].
    pub fn new_http(url: reqwest::Url, cache_size: usize) -> Self {
        let inner = RootProvider::new_http(url);
        Self::new(inner, cache_size)
    }

    /// Returns the latest L2 block number.
    pub async fn latest_block_number(&mut self) -> Result<u64, RpcError<TransportErrorKind>> {
        self.inner.get_block_number().await
    }

    /// Returns the chain ID.
    pub async fn chain_id(&mut self) -> Result<u64, RpcError<TransportErrorKind>> {
        self.inner.get_chain_id().await
    }
}

/// An error for the [AlloyChainProvider].
#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum AlloyChainProviderError {
    /// Transport error
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
    /// Block not found.
    #[error("Block not found: {0}")]
    BlockNotFound(BlockId),
    /// Failed to convert RPC receipts into consensus receipts.
    #[error("Failed to convert RPC receipts into consensus receipts {0}")]
    ReceiptsConversion(B256),
}

impl From<AlloyChainProviderError> for PipelineErrorKind {
    fn from(e: AlloyChainProviderError) -> Self {
        match e {
            AlloyChainProviderError::Transport(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("Transport error: {e}")),
            ),
            AlloyChainProviderError::BlockNotFound(id) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("L1 Block not found: {id}")),
            ),
            AlloyChainProviderError::ReceiptsConversion(_) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(
                    "Failed to convert RPC receipts into consensus receipts".to_string(),
                ))
            }
        }
    }
}

#[async_trait]
impl ChainProvider for AlloyChainProvider {
    type Error = AlloyChainProviderError;

    async fn header_by_hash(&mut self, hash: B256) -> Result<Header, Self::Error> {
        if let Some(header) = self.header_by_hash_cache.get(&hash) {
            return Ok(header.clone());
        }

        let block = self
            .inner
            .get_block_by_hash(hash)
            .await?
            .ok_or(AlloyChainProviderError::BlockNotFound(hash.into()))?;
        let header = block.header.into_consensus();

        self.header_by_hash_cache.put(hash, header.clone());

        Ok(header)
    }

    async fn block_info_by_number(&mut self, number: u64) -> Result<BlockInfo, Self::Error> {
        let block = self
            .inner
            .get_block_by_number(number.into())
            .await?
            .ok_or(AlloyChainProviderError::BlockNotFound(number.into()))?;
        let header = block.header.into_consensus();

        let block_info = BlockInfo {
            hash: header.hash_slow(),
            number,
            parent_hash: header.parent_hash,
            timestamp: header.timestamp,
        };
        Ok(block_info)
    }

    async fn receipts_by_hash(&mut self, hash: B256) -> Result<Vec<Receipt>, Self::Error> {
        if let Some(receipts) = self.receipts_by_hash_cache.get(&hash) {
            return Ok(receipts.clone());
        }

        let receipts = self
            .inner
            .get_block_receipts(hash.into())
            .await?
            .ok_or(AlloyChainProviderError::BlockNotFound(hash.into()))?;
        let consensus_receipts = receipts
            .into_iter()
            .map(|r| r.inner.into_primitives_receipt().as_receipt().cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or(AlloyChainProviderError::ReceiptsConversion(hash))?;

        self.receipts_by_hash_cache.put(hash, consensus_receipts.clone());
        Ok(consensus_receipts)
    }

    async fn block_info_and_transactions_by_hash(
        &mut self,
        hash: B256,
    ) -> Result<(BlockInfo, Vec<TxEnvelope>), Self::Error> {
        if let Some(block_info_and_txs) = self.block_info_and_transactions_by_hash_cache.get(&hash)
        {
            return Ok(block_info_and_txs.clone());
        }

        let block = self
            .inner
            .get_block_by_hash(hash)
            .full()
            .await?
            .ok_or(AlloyChainProviderError::BlockNotFound(hash.into()))?
            .into_consensus()
            .map_transactions(|t| t.inner.into_inner());

        let block_info = BlockInfo {
            hash: block.header.hash_slow(),
            number: block.header.number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        };

        self.block_info_and_transactions_by_hash_cache
            .put(hash, (block_info, block.body.transactions.clone()));

        Ok((block_info, block.body.transactions))
    }
}
