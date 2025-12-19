//! Follow client for querying sync status from an L2 CL RPC.

use alloy_eips::BlockNumberOrTag;
use alloy_provider::{Provider, RootProvider};
use async_trait::async_trait;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use kona_protocol::{BlockInfo, L2BlockInfo};
use kona_rpc::RollupNodeApiClient;
use std::time::Duration;
use thiserror::Error;
use url::Url;

/// Default timeout for follow client requests in milliseconds.
const DEFAULT_FOLLOW_TIMEOUT: u64 = 5000;

/// An error that occurred in the [`FollowClient`].
#[derive(Error, Debug)]
pub enum FollowClientError {
    /// An error occurred while building the HTTP client
    #[error("Failed to build HTTP client: {0}")]
    HttpClientBuild(String),

    /// An RPC error occurred
    #[error("RPC error: {0}")]
    RpcError(#[from] jsonrpsee::core::ClientError),

    /// An error occurred while fetching L1 block
    #[error("Failed to fetch L1 block: {0}")]
    L1BlockFetchError(String),

    /// Block not found
    #[error("Block not found")]
    BlockNotFound,
}

/// Simplified follow status containing only the essential sync information.
///
/// This struct contains a subset of fields from [`kona_protocol::SyncStatus`],
/// focusing only on the fields needed for following another rollup node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowStatus {
    /// The current L1 block that the derivation process is last idled at.
    pub current_l1: BlockInfo,
    /// The safe L2 block ref, derived from the L1 chain.
    pub safe_l2: L2BlockInfo,
    /// The finalized L2 block ref, derived from finalized L1 information.
    pub finalized_l2: L2BlockInfo,
}

/// Follow client trait for querying sync status from an L2 consensus layer RPC.
///
/// This trait defines the interface for communicating with another rollup node's
/// consensus layer to fetch its synchronization status and querying L1 blocks.
/// The main reason this trait exists is for mocking and unit testing.
#[async_trait]
pub trait FollowClient: Send + Sync {
    /// Gets the synchronization status from the follow source.
    ///
    /// Calls the `optimism_syncStatus` RPC method on the remote rollup node,
    /// extracts the essential fields, and returns a simplified status.
    ///
    /// # Returns
    ///
    /// Returns the [`FollowStatus`] containing only the current L1, safe L2,
    /// and finalized L2 blocks from the remote rollup node, or an error if
    /// the RPC call fails.
    async fn get_follow_status(&self) -> Result<FollowStatus, FollowClientError>;

    /// Fetches the L1 [`BlockInfo`] by block number.
    ///
    /// Queries the L1 execution layer for the block at the given number and
    /// returns the block information.
    ///
    /// # Arguments
    ///
    /// * `number` - The L1 block number to fetch
    ///
    /// # Returns
    ///
    /// Returns the [`BlockInfo`] for the requested L1 block, or an error if
    /// the block cannot be fetched or does not exist.
    async fn l1_block_info_by_number(&self, number: u64) -> Result<BlockInfo, FollowClientError>;
}

/// Builder for creating a [`HttpFollowClient`].
#[derive(Debug, Clone)]
pub struct FollowClientBuilder {
    /// The L2 consensus layer RPC URL.
    pub l2_url: Url,
    /// The L1 execution layer RPC URL.
    pub l1_url: Url,
    /// The timeout duration for requests.
    pub timeout: Duration,
}

impl FollowClientBuilder {
    /// Creates a new [`FollowClientBuilder`] with the given URLs.
    ///
    /// # Arguments
    ///
    /// * `l2_url` - The L2 consensus layer RPC endpoint URL
    /// * `l1_url` - The L1 execution layer RPC endpoint URL
    pub const fn new(l2_url: Url, l1_url: Url) -> Self {
        Self { l2_url, l1_url, timeout: Duration::from_millis(DEFAULT_FOLLOW_TIMEOUT) }
    }

    /// Sets the timeout duration for requests.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds the [`HttpFollowClient`].
    ///
    /// # Returns
    ///
    /// Returns a new `HttpFollowClient` instance or an error if the HTTP client
    /// cannot be built.
    pub fn build(self) -> Result<HttpFollowClient, FollowClientError> {
        // Build L2 CL client for sync status queries
        let l2_client = HttpClientBuilder::default()
            .request_timeout(self.timeout)
            .build(self.l2_url)
            .map_err(|e| FollowClientError::HttpClientBuild(e.to_string()))?;

        // Build L1 provider for block queries
        let l1_provider = RootProvider::new_http(self.l1_url);

        Ok(HttpFollowClient { l2_client, l1_provider })
    }
}

/// HTTP-based follow client for querying sync status from an L2 consensus layer RPC.
///
/// The [`HttpFollowClient`] wraps JSON-RPC clients to communicate with another
/// rollup node's consensus layer (L2) and the L1 execution layer.
#[derive(Clone, Debug)]
pub struct HttpFollowClient {
    /// The L2 consensus layer HTTP client for making RPC calls.
    l2_client: HttpClient,
    /// The L1 execution layer provider for querying blocks.
    l1_provider: RootProvider,
}

#[async_trait]
impl FollowClient for HttpFollowClient {
    async fn get_follow_status(&self) -> Result<FollowStatus, FollowClientError> {
        // Fetch the full sync status from the remote rollup node
        let sync_status = self.l2_client.op_sync_status().await?;

        // Extract only the fields we need
        Ok(FollowStatus {
            current_l1: sync_status.current_l1,
            safe_l2: sync_status.safe_l2,
            finalized_l2: sync_status.finalized_l2,
        })
    }

    async fn l1_block_info_by_number(&self, number: u64) -> Result<BlockInfo, FollowClientError> {
        // Fetch the block from L1
        let block = self
            .l1_provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .await
            .map_err(|e| FollowClientError::L1BlockFetchError(e.to_string()))?
            .ok_or(FollowClientError::BlockNotFound)?;

        // Convert to BlockInfo
        Ok(BlockInfo {
            hash: block.header.hash,
            number: block.header.number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        })
    }
}
