use super::{AuthenticationError, ClientError, metrics::Metrics};
use alloy_primitives::{B256, ChainId};
use alloy_rpc_types_engine::{Claims, JwtSecret};
use alloy_rpc_types_eth::BlockNumHash;
use async_trait::async_trait;
use jsonrpsee::{
    core::client::Subscription,
    ws_client::{HeaderMap, HeaderValue, WsClient, WsClientBuilder},
};
use kona_supervisor_metrics::observe_metrics_for_result_async;
use kona_supervisor_rpc::{BlockInfo, ManagedModeApiClient, jsonrpsee::SubscriptionTopic};
use kona_supervisor_types::{BlockSeal, OutputV0, Receipts, SubscriptionEvent};
use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};
use tokio::sync::Mutex;
use tracing::{error, info};

/// Trait for a managed node client that provides various methods to interact with the node.
#[async_trait]
pub trait ManagedNodeClient: Debug {
    /// Returns the [`ChainId`] of the managed node.
    async fn chain_id(&self) -> Result<ChainId, ClientError>;

    /// Subscribes to [`SubscriptionEvent`] from the managed node.
    async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ClientError>;

    /// Fetches [`Receipts`] for a given block hash.
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ClientError>;

    /// Fetches the [`OutputV0`] at a specific timestamp.
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ClientError>;

    /// Fetches the pending [`OutputV0`] at a specific timestamp.
    async fn pending_output_v0_at_timestamp(&self, timestamp: u64)
    -> Result<OutputV0, ClientError>;

    /// Fetches the L2 [`BlockInfo`] by timestamp.
    async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ClientError>;

    /// Fetches the [`BlockInfo`] by block number.
    async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ClientError>;

    /// Resets the managed node to the pre-interop state.
    async fn reset_pre_interop(&self) -> Result<(), ClientError>;

    /// Resets the node state with the provided block IDs.
    async fn reset(
        &self,
        unsafe_id: BlockNumHash,
        cross_unsafe_id: BlockNumHash,
        local_safe_id: BlockNumHash,
        cross_safe_id: BlockNumHash,
        finalised_id: BlockNumHash,
    ) -> Result<(), ClientError>;

    /// Invalidates a block in the managed node.
    async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ClientError>;

    /// Provides L1 [`BlockInfo`] to the managed node.
    async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ClientError>;

    /// Updates the finalized block ID in the managed node.
    async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ClientError>;

    /// Updates the cross-unsafe block ID in the managed node.
    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ClientError>;

    /// Updates the cross-safe block ID in the managed node.
    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ClientError>;

    /// Resets the ws-client to None when server disconnects
    async fn reset_ws_client(&self);
}

/// [`ClientConfig`] sets the configuration for the managed node client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The URL + port of the managed node
    pub url: String,
    /// The path to the JWT token for the managed node
    pub jwt_path: String,
}

impl ClientConfig {
    /// Reads the JWT secret from the configured file path.
    /// If the file cannot be read, falls back to creating a default JWT secret.
    pub fn jwt_secret(&self) -> Option<JwtSecret> {
        if let Ok(secret) = std::fs::read_to_string(&self.jwt_path) {
            return JwtSecret::from_hex(secret).ok();
        }
        Self::default_jwt_secret()
    }

    /// Uses the current directory to attempt to read
    /// the JWT secret from a file named `jwt.hex`.
    pub fn default_jwt_secret() -> Option<JwtSecret> {
        let cur_dir = std::env::current_dir().ok()?;
        if let Ok(secret) = std::fs::read_to_string(cur_dir.join("jwt.hex")).map_err(|err| {
            error!(
                target: "managed_node",
                %err,
                "Failed to read JWT file"
            );
        }) {
            return JwtSecret::from_hex(secret).ok();
        }
        None
    }
}

/// Client for interacting with a managed node.
#[derive(Debug)]
pub struct Client {
    config: ClientConfig,
    /// Chain ID of the managed node
    chain_id: OnceLock<ChainId>,
    /// The attached web socket client
    ws_client: Mutex<Option<Arc<WsClient>>>,
}

impl Client {
    /// Creates a new [`Client`] with the given configuration.
    pub fn new(config: ClientConfig) -> Self {
        Metrics::init(config.url.as_ref());
        Self { config, chain_id: OnceLock::new(), ws_client: Mutex::new(None) }
    }

    /// Creates authentication headers using JWT secret.
    fn create_auth_headers(&self) -> Result<HeaderMap, ClientError> {
        let Some(jwt_secret) = self.config.jwt_secret() else {
            error!(target: "managed_node", "JWT secret not found or invalid");
            return Err(AuthenticationError::InvalidJwt.into());
        };

        // Create JWT claims with current time
        let claims = Claims::with_current_timestamp();
        let token = jwt_secret.encode(&claims).map_err(|err| {
            error!(target: "managed_node", %err, "Failed to encode JWT claims");
            AuthenticationError::InvalidJwt
        })?;

        let mut headers = HeaderMap::new();
        let auth_header = format!("Bearer {}", token);

        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_header).map_err(|err| {
                error!(target: "managed_node", %err, "Invalid authorization header");
                AuthenticationError::InvalidHeader
            })?,
        );

        Ok(headers)
    }

    /// Returns a reference to the WebSocket client, creating it if it doesn't exist.
    // todo: support http client as well
    pub async fn get_ws_client(&self) -> Result<Arc<WsClient>, ClientError> {
        let mut ws_client_guard = self.ws_client.lock().await;
        if ws_client_guard.is_none() {
            let headers = self.create_auth_headers().inspect_err(|err| {
                error!(target: "managed_node", %err, "Failed to create auth headers");
            })?;

            info!(target: "managed_node", ws_url = self.config.url, "Creating a new web socket client");
            let client =
                WsClientBuilder::default().set_headers(headers).build(&self.config.url).await?;

            *ws_client_guard = Some(Arc::new(client));
        }
        Ok(ws_client_guard.clone().unwrap())
    }
}

#[async_trait]
impl ManagedNodeClient for Client {
    async fn reset_ws_client(&self) {
        let mut ws_client_guard = self.ws_client.lock().await;
        if ws_client_guard.is_some() {
            *ws_client_guard = None;
        };
    }
    async fn chain_id(&self) -> Result<ChainId, ClientError> {
        if let Some(chain_id) = self.chain_id.get() {
            return Ok(*chain_id);
        }

        let client = self.get_ws_client().await?;
        let chain_id_str = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "chain_id",
            async {
              client.chain_id().await
            },
            "node" => self.config.url.clone()
        )
        .inspect_err(|err| {
            error!(target: "managed_node", %err, "Failed to get chain ID");
        })?;

        let chain_id = chain_id_str.parse::<u64>().inspect_err(|err| {
            error!(target: "managed_node", %err, "Failed to parse chain ID");
        })?;

        let _ = self.chain_id.set(chain_id);
        Ok(chain_id)
    }

    async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ClientError> {
        let client = self.get_ws_client().await?; // This returns ManagedNodeError, handled by your function
        let subscription = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "subscribe_events",
            async {
              ManagedModeApiClient::subscribe_events(client.as_ref(), SubscriptionTopic::Events).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(subscription)
    }

    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ClientError> {
        let client = self.get_ws_client().await?; // This returns ManagedNodeError, handled by your function
        let receipts = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "fetch_receipts",
            async {
              ManagedModeApiClient::fetch_receipts(client.as_ref(), block_hash).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(receipts)
    }

    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ClientError> {
        let client = self.get_ws_client().await?;
        let output_v0 = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "output_v0_at_timestamp",
            async {
              ManagedModeApiClient::output_v0_at_timestamp(client.as_ref(), timestamp).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(output_v0)
    }

    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ClientError> {
        let client = self.get_ws_client().await?;
        let output_v0 = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "pending_output_v0_at_timestamp",
            async {
              ManagedModeApiClient::pending_output_v0_at_timestamp(client.as_ref(), timestamp).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(output_v0)
    }

    async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ClientError> {
        let client = self.get_ws_client().await?;
        let block_info = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "l2_block_ref_by_timestamp",
            async {
              ManagedModeApiClient::l2_block_ref_by_timestamp(client.as_ref(), timestamp).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(block_info)
    }

    async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ClientError> {
        let client = self.get_ws_client().await?;
        let block_info = observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "block_ref_by_number",
            async {
              ManagedModeApiClient::l2_block_ref_by_number(client.as_ref(), block_number).await
            },
            "node" => self.config.url.clone()
        )?;

        Ok(block_info)
    }

    async fn reset_pre_interop(&self) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "reset_pre_interop",
            async {
              ManagedModeApiClient::reset_pre_interop(client.as_ref()).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn reset(
        &self,
        unsafe_id: BlockNumHash,
        cross_unsafe_id: BlockNumHash,
        local_safe_id: BlockNumHash,
        cross_safe_id: BlockNumHash,
        finalised_id: BlockNumHash,
    ) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "reset",
            async {
              ManagedModeApiClient::reset(client.as_ref(), unsafe_id, cross_unsafe_id, local_safe_id, cross_safe_id, finalised_id).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "invalidate_block",
            async {
              ManagedModeApiClient::invalidate_block(client.as_ref(), seal).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "provide_l1",
            async {
              ManagedModeApiClient::provide_l1(client.as_ref(), block_info).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "update_finalized",
            async {
              ManagedModeApiClient::update_finalized(client.as_ref(), finalized_block_id).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "update_cross_unsafe",
            async {
              ManagedModeApiClient::update_cross_unsafe(client.as_ref(), cross_unsafe_block_id).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }

    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ClientError> {
        let client = self.get_ws_client().await?;
        observe_metrics_for_result_async!(
            Metrics::MANAGED_NODE_RPC_REQUESTS_SUCCESS_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUESTS_ERROR_TOTAL,
            Metrics::MANAGED_NODE_RPC_REQUEST_DURATION_SECONDS,
            "update_cross_safe",
            async {
              ManagedModeApiClient::update_cross_safe(client.as_ref(), derived_block_id, source_block_id).await
            },
            "node" => self.config.url.clone()
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_mock_jwt_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        // Create a valid 32-byte hex string for JWT secret
        let hex_secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        writeln!(file, "{}", hex_secret).expect("Failed to write to temp file");
        file
    }

    #[tokio::test]
    async fn test_jwt_secret_functionality() {
        // Test with valid JWT file
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = ClientConfig {
            url: "test.server".to_string(),
            jwt_path: jwt_path.to_str().unwrap().to_string(),
        };

        let jwt_secret = config.jwt_secret();
        assert!(jwt_secret.is_some(), "JWT secret should be loaded from file");

        // Test with invalid path - should now return None instead of creating a file
        let config_invalid = ClientConfig {
            url: "test.server".to_string(),
            jwt_path: "/nonexistent/path/jwt.hex".to_string(),
        };

        let jwt_secret_fallback = config_invalid.jwt_secret();
        assert!(jwt_secret_fallback.is_none(), "Should return None when JWT file doesn't exist");

        // Test default_jwt_secret with nonexistent file
        let original_dir = std::env::current_dir().expect("Should get current directory");

        // Change to a temporary directory where jwt.hex doesn't exist
        let temp_dir = tempfile::tempdir().expect("Should create temp directory");
        std::env::set_current_dir(temp_dir.path()).expect("Should change directory");

        let default_secret = ClientConfig::default_jwt_secret();
        assert!(
            default_secret.is_none(),
            "default_jwt_secret should return None when jwt.hex doesn't exist"
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).expect("Should restore directory");
    }
}
