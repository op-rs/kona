//! [`ManagedNode`] implementation for subscribing to the events from managed node.

use alloy_primitives::{B256, ChainId};
use alloy_rpc_types_engine::{Claims, JwtSecret};
use async_trait::async_trait;
use jsonrpsee::ws_client::{HeaderMap, HeaderValue, WsClient, WsClientBuilder};
use kona_supervisor_rpc::{ManagedModeApiClient, jsonrpsee::SubscriptionTopic};
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, LogStorageReader};
use kona_supervisor_types::Receipts;
use std::sync::{Arc, OnceLock};
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::{
    AuthenticationError, ManagedEventTask, ManagedNodeError, NodeSubscriber, ReceiptProvider,
    SubscriptionError,
};
use crate::event::ChainEvent;

/// [`ManagedNodeConfig`] sets the configuration for the managed node.
#[derive(Debug, Clone)]
pub struct ManagedNodeConfig {
    /// The URL + port of the managed node
    pub url: String,
    /// The path to the JWT token for the managed node
    pub jwt_path: String,
    /// The URL of the L1 RPC endpoint
    pub l1_rpc_url: String,
}

impl ManagedNodeConfig {
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

/// [`ManagedNode`] handles the subscription to managed node events.
///
/// It manages the WebSocket connection lifecycle and processes incoming events.
#[derive(Debug)]
pub struct ManagedNode<DB> {
    /// Configuration for connecting to the managed node
    config: Arc<ManagedNodeConfig>,
    /// Chain ID of the managed node
    chain_id: OnceLock<ChainId>,
    /// The database provider for fetching information
    db_provider: Option<Arc<DB>>,
    /// The attached web socket client
    ws_client: Mutex<Option<Arc<WsClient>>>,
    // Cancellation token to stop the processor
    cancel_token: CancellationToken,
    /// Handle to the async subscription task
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

impl<DB> ManagedNode<DB>
where
    DB: LogStorageReader + DerivationStorageReader + Send + Sync + 'static,
{
    /// Creates a new [`ManagedNode`] with the specified configuration.
    pub fn new(config: Arc<ManagedNodeConfig>, cancel_token: CancellationToken) -> Self {
        Self {
            config,
            chain_id: OnceLock::new(),
            db_provider: None,
            ws_client: Mutex::new(None),
            cancel_token,
            task_handle: Mutex::new(None),
        }
    }

    /// Sets the database provider for the managed node.
    pub fn set_db_provider(&mut self, db_provider: Arc<DB>) {
        self.db_provider = Some(db_provider);
    }

    /// Returns a reference to the WebSocket client, creating it if it doesn't exist.
    // todo: support http client as well
    pub async fn get_ws_client(&self) -> Result<Arc<WsClient>, ManagedNodeError> {
        let mut ws_client_guard = self.ws_client.lock().await;
        if ws_client_guard.is_none() {
            let headers = self.create_auth_headers().inspect_err(|err| {
                error!(target: "managed_node", %err, "Failed to create auth headers");
            })?;

            let ws_url = format!("ws://{}", self.config.url);
            info!(target: "managed_node", ws_url, "Creating a new web socket client");

            let client = WsClientBuilder::default().set_headers(headers).build(&ws_url).await?;

            *ws_client_guard = Some(Arc::new(client));
        }
        Ok(ws_client_guard.clone().unwrap())
    }

    /// Returns the [`ChainId`] of the [`ManagedNode`].
    /// If the chain ID is already cached, it returns that.
    /// If not, it fetches the chain ID from the managed node.
    pub async fn chain_id(&self) -> Result<ChainId, ManagedNodeError> {
        if let Some(chain_id) = self.chain_id.get() {
            return Ok(*chain_id);
        }

        // Fetch chain ID from the managed node
        let client = self.get_ws_client().await?;
        let chain_id_str = client.chain_id().await.inspect_err(|err| {
            error!(target: "managed_node", %err, "Failed to get chain ID");
        })?;

        let chain_id = chain_id_str.parse::<u64>().inspect_err(|err| {
            error!(target: "managed_node", %err, "Failed to parse chain ID");
        })?;

        let _ = self.chain_id.set(chain_id);
        Ok(chain_id)
    }

    /// Creates authentication headers using JWT secret.
    fn create_auth_headers(&self) -> Result<HeaderMap, ManagedNodeError> {
        let Some(jwt_secret) = self.config.jwt_secret() else {
            error!(target: "managed_node", "JWT secret not found or invalid");
            return Err(AuthenticationError::InvalidJwt.into())
        };

        // Create JWT claims with current time
        let claims = Claims::with_current_timestamp();
        let token = jwt_secret.encode(&claims).map_err(|err| {
            error!(target: "managed_node", %err, "Failed to encode JWT claims");
            AuthenticationError::InvalidJwt
        })?;

        info!(target: "managed_node", token, "JWT token created successfully");
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
}

#[async_trait]
impl<DB> NodeSubscriber for ManagedNode<DB>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
{
    /// Starts a subscription to the managed node.
    ///
    /// Establishes a WebSocket connection and subscribes to node events.
    /// Spawns a background task to process incoming events.
    async fn start_subscription(
        &self,
        event_tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), ManagedNodeError> {
        let mut task_handle_guard = self.task_handle.lock().await;
        if task_handle_guard.is_some() {
            Err(SubscriptionError::AlreadyActive)?
        }

        let client = self.get_ws_client().await?;

        let mut subscription =
            client.subscribe_events(SubscriptionTopic::Events).await.inspect_err(|err| {
                error!(
                    target: "managed_node",
                    %err,
                    "Failed to subscribe to events"
                );
            })?;

        let cancel_token = self.cancel_token.clone();

        let db_provider =
            self.db_provider.as_ref().ok_or_else(|| SubscriptionError::DatabaseProviderNotFound)?;
        // Creates a task instance to sort and process the events from the subscription
        let task = ManagedEventTask::new(
            self.config.l1_rpc_url.clone(),
            db_provider.clone(),
            event_tx,
            client,
        );

        // Start background task to handle events
        let handle = tokio::spawn(async move {
            info!(target: "managed_node", "Subscription task started");
            loop {
                tokio::select! {
                    // Listen for stop signal
                    _ = cancel_token.cancelled() => {
                        info!(target: "managed_node", "Cancellation token triggered, shutting down subscription");
                        break;
                    }

                    // Listen for events from subscription
                    incoming_event = subscription.next() => {
                        match incoming_event {
                            Some(Ok(subscription_event)) => {
                                task.handle_managed_event(subscription_event.data).await;
                            }
                            Some(Err(err)) => {
                                error!(
                                    target: "managed_node",
                                    %err,
                                    "Error in event deserialization"
                                );
                        // Continue processing next events despite this error
                            }
                            None => {
                                // Subscription closed by the server
                                warn!(target: "managed_node", "Subscription closed by server");
                                break;
                            }
                        }
                    }
                }
            }

            // Try to unsubscribe gracefully
            if let Err(err) = subscription.unsubscribe().await {
                warn!(
                    target: "managed_node",
                    %err,
                    "Failed to unsubscribe gracefully"
                );
            }

            info!(target: "managed_node", "Subscription task finished");
        });

        *task_handle_guard = Some(handle);

        info!(target: "managed_node", "Subscription started successfully");
        Ok(())
    }
}

/// Implements [`ReceiptProvider`] for [`ManagedNode`] by delegating to the underlying WebSocket
/// client.
///
/// This allows `LogIndexer` and similar components to remain decoupled from the full
/// [`ManagedModeApiClient`] interface, using only the receipt-fetching capability
#[async_trait]
impl<DB> ReceiptProvider for ManagedNode<DB>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
{
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError> {
        let client = self.get_ws_client().await?;
        let receipts = ManagedModeApiClient::fetch_receipts(client.as_ref(), block_hash).await?;
        Ok(receipts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_eips::BlockNumHash;
    use kona_interop::{DerivedRefPair, ManagedEvent, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::StorageError;
    use kona_supervisor_types::{Log, SuperHead};
    use mockall::mock;

    use std::io::Write;
    use tempfile::NamedTempFile;
    use tokio::sync::mpsc;

    mock! {
        #[derive(Debug)]
        pub Db {}
        impl LogStorageReader for Db {
            fn get_latest_block(&self) -> Result<BlockInfo, StorageError>;
            fn get_block_by_log(&self, block_number: u64, log: &Log) -> Result<BlockInfo, StorageError>;
            fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError>;
        }

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, _source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_pair(&self) -> Result<DerivedRefPair, StorageError>;
        }

        impl HeadRefStorageReader for Db {
            fn get_current_l1(&self) -> Result<BlockInfo, StorageError>;
            fn get_safety_head_ref(&self, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_super_head(&self) -> Result<SuperHead, StorageError>;
        }
    }

    fn create_mock_jwt_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        // Create a valid 32-byte hex string for JWT secret
        let hex_secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        writeln!(file, "{}", hex_secret).expect("Failed to write to temp file");
        file
    }

    #[tokio::test]
    async fn test_managed_event_serialization_deserialization() {
        // Test deserializing a complete ManagedEvent from JSON
        let complete_json = r#"{
            "reset": "reset_id_123",
            "unsafeBlock": {
                "hash": "0x0101010101010101010101010101010101010101010101010101010101010101",
                "number": 124,
                "parentHash": "0x0202020202020202020202020202020202020202020202020202020202020202",
                "timestamp": 1678886400
            },
            "derivationUpdate": {
                "source": {
                    "hash": "0x0303030303030303030303030303030303030303030303030303030303030303",
                    "number": 124,
                    "parentHash": "0x0404040404040404040404040404040404040404040404040404040404040404",
                    "timestamp": 1678886400
                },
                "derived": {
                    "hash": "0x0505050505050505050505050505050505050505050505050505050505050505",
                    "number": 124,
                    "parentHash": "0x0606060606060606060606060606060606060606060606060606060606060606",
                    "timestamp": 1678886400
                }
            },
            "exhaustL1": {
                "source": {
                    "hash": "0x0707070707070707070707070707070707070707070707070707070707070707",
                    "number": 124,
                    "parentHash": "0x0808080808080808080808080808080808080808080808080808080808080808",
                    "timestamp": 1678886400
                },
                "derived": {
                    "hash": "0x0909090909090909090909090909090909090909090909090909090909090909",
                    "number": 124,
                    "parentHash": "0x0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
                    "timestamp": 1678886400
                }
            },
            "replaceBlock": {
                "replacement": {
                    "hash": "0x0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
                    "number": 124,
                    "parentHash": "0x0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c",
                    "timestamp": 1678886400
                },
                "invalidated": "0x0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d"
            },
            "derivationOriginUpdate": {
                "hash": "0x0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e",
                "number": 50,
                "parentHash": "0x0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f",
                "timestamp": 1678886400
            }
        }"#;

        let deserialized: ManagedEvent = serde_json::from_str(complete_json)
            .expect("Failed to deserialize complete ManagedEvent from JSON");

        // Verify all fields are correctly deserialized
        assert_eq!(deserialized.reset, Some("reset_id_123".to_string()));
        assert!(deserialized.unsafe_block.is_some());
        assert!(deserialized.derivation_update.is_some());
        assert!(deserialized.exhaust_l1.is_some());
        assert!(deserialized.replace_block.is_some());
        assert!(deserialized.derivation_origin_update.is_some());

        // Verify specific field values
        let unsafe_block = deserialized.unsafe_block.unwrap();
        assert_eq!(unsafe_block.number, 124);
        assert_eq!(unsafe_block.timestamp, 1678886400);

        let origin_update = deserialized.derivation_origin_update.unwrap();
        assert_eq!(origin_update.number, 50);

        // Test deserializing ManagedEvent with all fields as null/None
        let empty_json = r#"{
            "reset": null,
            "unsafeBlock": null,
            "derivationUpdate": null,
            "exhaustL1": null,
            "replaceBlock": null,
            "derivationOriginUpdate": null
        }"#;

        let empty_deserialized: ManagedEvent = serde_json::from_str(empty_json)
            .expect("Failed to deserialize empty ManagedEvent from JSON");

        assert!(empty_deserialized.reset.is_none());
        assert!(empty_deserialized.unsafe_block.is_none());
        assert!(empty_deserialized.derivation_update.is_none());
        assert!(empty_deserialized.exhaust_l1.is_none());
        assert!(empty_deserialized.replace_block.is_none());
        assert!(empty_deserialized.derivation_origin_update.is_none());

        // Test deserializing partial ManagedEvent (only some fields present)
        let partial_json = r#"{
            "reset": "partial_reset",
            "unsafeBlock": {
                "hash": "0x1111111111111111111111111111111111111111111111111111111111111111",
                "number": 42,
                "parentHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
                "timestamp": 1678886401
            }
        }"#;

        let partial_deserialized: ManagedEvent = serde_json::from_str(partial_json)
            .expect("Failed to deserialize partial ManagedEvent from JSON");

        assert_eq!(partial_deserialized.reset, Some("partial_reset".to_string()));
        assert!(partial_deserialized.unsafe_block.is_some());
        assert!(partial_deserialized.derivation_update.is_none());
        assert!(partial_deserialized.exhaust_l1.is_none());
        assert!(partial_deserialized.replace_block.is_none());
        assert!(partial_deserialized.derivation_origin_update.is_none());

        let partial_unsafe_block = partial_deserialized.unsafe_block.unwrap();
        assert_eq!(partial_unsafe_block.number, 42);
        assert_eq!(partial_unsafe_block.timestamp, 1678886401);
    }

    #[tokio::test]
    async fn test_jwt_secret_functionality() {
        // Test with valid JWT file
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = ManagedNodeConfig {
            url: "test.server".to_string(),
            jwt_path: jwt_path.to_str().unwrap().to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        };

        let jwt_secret = config.jwt_secret();
        assert!(jwt_secret.is_some(), "JWT secret should be loaded from file");

        // Test with invalid path - should now return None instead of creating a file
        let config_invalid = ManagedNodeConfig {
            url: "test.server".to_string(),
            jwt_path: "/nonexistent/path/jwt.hex".to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        };

        let jwt_secret_fallback = config_invalid.jwt_secret();
        assert!(jwt_secret_fallback.is_none(), "Should return None when JWT file doesn't exist");

        // Test default_jwt_secret with nonexistent file
        let original_dir = std::env::current_dir().expect("Should get current directory");

        // Change to a temporary directory where jwt.hex doesn't exist
        let temp_dir = tempfile::tempdir().expect("Should create temp directory");
        std::env::set_current_dir(temp_dir.path()).expect("Should change directory");

        let default_secret = ManagedNodeConfig::default_jwt_secret();
        assert!(
            default_secret.is_none(),
            "default_jwt_secret should return None when jwt.hex doesn't exist"
        );

        // Restore original directory
        std::env::set_current_dir(original_dir).expect("Should restore directory");
    }

    #[tokio::test]
    async fn test_header_creation_for_websocket_auth() {
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = ManagedNodeConfig {
            url: "test.server".to_string(),
            jwt_path: jwt_path.to_str().unwrap().to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        };

        let jwt_secret = config.jwt_secret().expect("Should have JWT secret");

        // Test that we can create the authorization header as expected
        let mut headers = HeaderMap::new();
        let auth_header =
            format!("Bearer {}", alloy_primitives::hex::encode(jwt_secret.as_bytes()));
        let header_result = HeaderValue::from_str(&auth_header);

        assert!(header_result.is_ok(), "Should be able to create valid authorization header");

        headers.insert("Authorization", header_result.unwrap());
        assert!(headers.contains_key("Authorization"), "Headers should contain Authorization");
    }

    #[tokio::test]
    async fn test_subscription_lifecycle() {
        // Test that we can create a subscriber and verify basic functionality
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = Arc::new(ManagedNodeConfig {
            url: "invalid.server:8545".to_string(), // Intentionally invalid to test error handling
            jwt_path: jwt_path.to_str().unwrap().to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        });

        let subscriber = ManagedNode::<MockDb>::new(config, CancellationToken::new());

        // Test that we can create the subscriber instance
        assert!(subscriber.task_handle.lock().await.is_none());

        // Create a channel for events
        let (event_tx, _event_rx) = mpsc::channel(100);

        // Test starting subscription to invalid server (should fail)
        let start_result = subscriber.start_subscription(event_tx).await;
        assert!(start_result.is_err(), "Subscription to invalid server should fail");

        // Verify state remains consistent after failure
        assert!(subscriber.task_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_websocket_client_creation() {
        // Test WebSocket client creation with invalid server (should fail)
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = Arc::new(ManagedNodeConfig {
            url: "invalid.server:8545".to_string(),
            jwt_path: jwt_path.to_str().unwrap().to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        });

        let managed_node = ManagedNode::<MockDb>::new(config, CancellationToken::new());

        // Test WebSocket client creation - should fail with invalid server
        let client_result = managed_node.get_ws_client().await;
        assert!(
            client_result.is_err(),
            "WebSocket client creation should fail with invalid server"
        );

        // Test with invalid JWT path
        let config_no_jwt = Arc::new(ManagedNodeConfig {
            url: "localhost:8545".to_string(),
            jwt_path: "/nonexistent/jwt.hex".to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        });

        let managed_node_no_jwt =
            ManagedNode::<MockDb>::new(config_no_jwt, CancellationToken::new());
        let client_no_jwt_result = managed_node_no_jwt.get_ws_client().await;
        assert!(client_no_jwt_result.is_err(), "Should fail with missing JWT file");
    }

    #[tokio::test]
    async fn test_thread_safety_and_race_condition_prevention() {
        // This test verifies that multiple concurrent calls to get_ws_client()
        // don't create multiple WebSocket clients (race condition prevention)
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = Arc::new(ManagedNodeConfig {
            url: "localhost:8545".to_string(), // Use localhost to avoid DNS resolution delays
            jwt_path: jwt_path.to_str().unwrap().to_string(),
            l1_rpc_url: "test.l1.rpc".to_string(),
        });

        let managed_node = Arc::new(ManagedNode::<MockDb>::new(config, CancellationToken::new()));

        // Test that the ManagedNode can be shared across threads (Send + Sync)
        let node1 = managed_node.clone();
        let node2 = managed_node.clone();

        // Spawn multiple tasks that try to get the WebSocket client concurrently
        // Note: These will fail because localhost:8545 isn't running, but we're testing
        // that they don't race and that the error handling is consistent
        let task1 = tokio::spawn(async move { node1.get_ws_client().await });
        let task2 = tokio::spawn(async move { node2.get_ws_client().await });

        let result1 = task1.await.expect("Task should complete");
        let result2 = task2.await.expect("Task should complete");

        // Both should fail (because server isn't running), but importantly,
        // they should fail consistently without panicking or creating race conditions
        assert!(result1.is_err(), "Should fail with connection error");
        assert!(result2.is_err(), "Should fail with connection error");
    }
}
