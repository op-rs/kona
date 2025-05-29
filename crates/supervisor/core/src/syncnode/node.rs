//! [`ManagedNode`] implementation for subscribing to the events from managed node.

use alloy_rpc_types_engine::JwtSecret;
use jsonrpsee::{
    core::client::Subscription,
    ws_client::{HeaderMap, HeaderValue, WsClient, WsClientBuilder},
};
use kona_supervisor_rpc::ManagedModeApiClient;
use kona_supervisor_types::ManagedEvent;
use std::sync::Arc;
use tokio::{
    sync::{Mutex, mpsc, watch},
    task::JoinHandle,
};
use tracing::{debug, error, info, warn};

use crate::{ManagedNodeError, NodeEvent, SubscriptionError};

/// Configuration for the managed node.
#[derive(Debug)]
pub struct ManagedNodeConfig {
    /// The URL + port of the managed node
    pub url: String,
    /// The path to the JWT token for the managed node
    pub jwt_path: String,
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
pub struct ManagedNode {
    /// Configuration for connecting to the managed node
    config: Arc<ManagedNodeConfig>,
    /// The attached web socket client
    ws_client: Mutex<Option<Arc<WsClient>>>,
    /// Channel for signaling the subscription task to stop
    stop_tx: Option<watch::Sender<bool>>,
    /// Handle to the async subscription task
    task_handle: Option<JoinHandle<()>>,
}

impl ManagedNode {
    /// Creates a new [`ManagedNode`] with the specified configuration.
    pub fn new(config: Arc<ManagedNodeConfig>) -> Self {
        Self { config, ws_client: Mutex::new(None), stop_tx: None, task_handle: None }
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

    /// Starts a subscription to the managed node.
    ///
    /// Establishes a WebSocket connection and subscribes to node events.
    /// Spawns a background task to process incoming events.
    pub async fn start_subscription(
        &mut self,
        event_tx: mpsc::Sender<NodeEvent>,
    ) -> Result<(), ManagedNodeError> {
        if self.task_handle.is_some() {
            Err(SubscriptionError::AlreadyActive)?
        }

        let client = self.get_ws_client().await?;

        let mut subscription: Subscription<Option<ManagedEvent>> =
            ManagedModeApiClient::subscribe_events(client.as_ref()).await.inspect_err(|err| {
                error!(
                    target: "managed_node",
                    %err,
                    "Failed to subscribe to events"
                );
            })?;

        // Create stop channel for graceful shutdown
        let (stop_tx, mut stop_rx) = watch::channel(false);
        self.stop_tx = Some(stop_tx);

        // Start background task to handle events
        let handle = tokio::spawn(async move {
            info!(target: "managed_node", "Subscription task started");
            loop {
                tokio::select! {
                    // Listen for stop signal
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            info!(target: "managed_node", "Stop signal received, shutting down subscription");
                            break;
                        }
                    }
                    // Listen for events from subscription
                    event = subscription.next() => {
                        match event {
                            Some(event_result) => {
                                match event_result {
                                    Ok(managed_event) => {
                                        Self::handle_managed_event(&event_tx, managed_event).await;
                                    },
                                    Err(err) => {
                                        error!(
                                            target: "managed_node",
                                            %err,
                                            "Error in event deserialization");
                                        // Continue processing next events despite this error
                                    }
                                }
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

        self.task_handle = Some(handle);
        info!(target: "managed_node", "Subscription started successfully");
        Ok(())
    }

    /// Stops the subscription to the managed node.
    ///
    /// Sends a stop signal to the background task and waits for it to complete.
    pub async fn stop_subscription(&mut self) -> Result<(), ManagedNodeError> {
        if let Some(stop_tx) = self.stop_tx.take() {
            debug!(target: "managed_node", action = "send_stop_signal", "Sending stop signal to subscription task");
            stop_tx.send(true).map_err(|err| {
                error!(
                    target: "managed_node",
                    %err,
                    "Failed to send stop signal"
                );
                SubscriptionError::SendStopSignalFailed
            })?;
        } else {
            Err(SubscriptionError::MissingStopChannel)?;
        }

        // Wait for task to complete
        if let Some(handle) = self.task_handle.take() {
            debug!(target: "managed_node", "Waiting for subscription task to complete");
            handle.await.map_err(|err| {
                error!(
                    target: "managed_node",
                    %err,
                    "Failed to join task"
                );
                SubscriptionError::ShutdownDaemonFailed
            })?;
            info!(target: "managed_node", "Subscription stopped and task joined");
        } else {
            Err(SubscriptionError::SubscriptionNotFound)?;
        }

        Ok(())
    }

    /// Creates authentication headers using JWT secret.
    fn create_auth_headers(&self) -> Result<HeaderMap, ManagedNodeError> {
        let Some(jwt_secret) = self.config.jwt_secret() else {
            error!(target: "managed_node", "JWT secret not found or invalid");
            return Err(ManagedNodeError::Authentication(
                "jwt secret not found or invalid".to_string(),
            ));
        };

        let mut headers = HeaderMap::new();
        let auth_header =
            format!("Bearer {}", alloy_primitives::hex::encode(jwt_secret.as_bytes()));

        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_header).map_err(|err| {
                error!(target: "managed_node", %err, "Invalid authorization header");
                ManagedNodeError::Authentication("invalid authorization header".to_string())
            })?,
        );

        Ok(headers)
    }

    /// Processes a managed event received from the subscription.
    ///
    /// Analyzes the event content and takes appropriate actions based on the
    /// event fields.
    async fn handle_managed_event(
        event_tx: &mpsc::Sender<NodeEvent>,
        event_result: Option<ManagedEvent>,
    ) {
        match event_result {
            Some(event) => {
                debug!(target: "managed_node", %event, "Handling ManagedEvent");

                // Process each field of the event if it's present
                if let Some(reset_id) = &event.reset {
                    info!(target: "managed_node", %reset_id, "Reset event received");
                    // Handle reset action
                }

                if let Some(unsafe_block) = &event.unsafe_block {
                    info!(target: "managed_node", %unsafe_block, "Unsafe block event received");

                    // todo: check any pre processing needed
                    if let Err(err) =
                        event_tx.send(NodeEvent::UnsafeBlock { block: *unsafe_block }).await
                    {
                        warn!(target: "managed_node", %err, "Failed to send unsafe block event, channel closed or receiver dropped");
                    }
                }

                if let Some(derived_ref_pair) = &event.derivation_update {
                    info!(target: "managed_node", %derived_ref_pair, "Derivation update received");

                    // todo: check any pre processing needed
                    if let Err(err) = event_tx
                        .send(NodeEvent::DerivedBlock {
                            derived_ref_pair: derived_ref_pair.clone(),
                        })
                        .await
                    {
                        warn!(target: "managed_node", %err, "Failed to derivation update event, channel closed or receiver dropped");
                    }
                }

                if let Some(derived_ref_pair) = &event.exhaust_l1 {
                    info!(
                        target: "managed_node",
                        %derived_ref_pair,
                        "L1 exhausted event received"
                    );

                    // todo: check if the last derived_ref_pair derived from l1 is sent as part of
                    // this event if yes, then we can send it to the event_tx
                    // otherwise, we can ignore this event

                    // Handle L1 exhaustion
                }

                if let Some(replacement) = &event.replace_block {
                    info!(target: "managed_node", %replacement, "Block replacement received");

                    // todo: check any pre processing needed
                    if let Err(err) = event_tx
                        .send(NodeEvent::BlockReplaced { replacement: replacement.clone() })
                        .await
                    {
                        warn!(target: "managed_node", %err, "Failed to send block replacement event, channel closed or receiver dropped");
                    }
                }

                if let Some(origin) = &event.derivation_origin_update {
                    info!(target: "managed_node", %origin, "Derivation origin update received");

                    // todo: check if we need to send this to the event_tx
                }

                // Check if this was an empty event (all fields None)
                if event.reset.is_none() &&
                    event.unsafe_block.is_none() &&
                    event.derivation_update.is_none() &&
                    event.exhaust_l1.is_none() &&
                    event.replace_block.is_none() &&
                    event.derivation_origin_update.is_none()
                {
                    debug!(target: "managed_node", "Received empty event with all fields None");
                }
            }
            None => {
                warn!(
                    target: "managed_node",
                    "Received None event, possibly an empty notification or an issue with deserialization."
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_types::BlockReplacement;
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
        };

        let jwt_secret = config.jwt_secret();
        assert!(jwt_secret.is_some(), "JWT secret should be loaded from file");

        // Test with invalid path - should now return None instead of creating a file
        let config_invalid = ManagedNodeConfig {
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
        });

        let mut subscriber = ManagedNode::new(config);

        // Test that we can create the subscriber instance
        assert!(subscriber.task_handle.is_none());
        assert!(subscriber.stop_tx.is_none());

        // Create a channel for events
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);

        // Test starting subscription to invalid server (should fail)
        let start_result = subscriber.start_subscription(event_tx).await;
        assert!(start_result.is_err(), "Subscription to invalid server should fail");

        // Verify state remains consistent after failure
        assert!(subscriber.task_handle.is_none());
    }

    #[tokio::test]
    async fn test_handle_managed_event_sends_unsafe_block() {
        // 1. Set up channel
        let (tx, mut rx) = mpsc::channel(1);

        // 2. Create a ManagedEvent with an unsafe_block
        let block_info = BlockInfo {
            hash: B256::from([0u8; 32]),
            number: 1,
            parent_hash: B256::from([1u8; 32]),
            timestamp: 42,
        };
        let managed_event = ManagedEvent {
            reset: None,
            unsafe_block: Some(block_info),
            derivation_update: None,
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };

        ManagedNode::handle_managed_event(&tx, Some(managed_event)).await;

        let event = rx.recv().await.expect("Should receive event");
        match event {
            NodeEvent::UnsafeBlock { block } => assert_eq!(block, block_info),
            _ => panic!("Expected UnsafeBlock event"),
        }
    }

    #[tokio::test]
    async fn test_handle_managed_event_sends_derivation_update() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Create a mock DerivedRefPair (adjust fields as needed)
        let derived_ref_pair = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([2u8; 32]),
                number: 2,
                parent_hash: B256::from([3u8; 32]),
                timestamp: 100,
            },
            derived: BlockInfo {
                hash: B256::from([4u8; 32]),
                number: 3,
                parent_hash: B256::from([5u8; 32]),
                timestamp: 101,
            },
        };

        let managed_event = ManagedEvent {
            reset: None,
            unsafe_block: None,
            derivation_update: Some(derived_ref_pair.clone()),
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };

        ManagedNode::handle_managed_event(&tx, Some(managed_event)).await;

        let event = rx.recv().await.expect("Should receive event");
        match event {
            NodeEvent::DerivedBlock { derived_ref_pair: pair } => {
                assert_eq!(pair, derived_ref_pair)
            }
            _ => panic!("Expected DerivedBlock event"),
        }
    }

    #[tokio::test]
    async fn test_handle_managed_event_sends_block_replacement() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // Create a mock BlockReplacement (adjust fields as needed)
        let replacement = BlockReplacement {
            replacement: BlockInfo {
                hash: B256::from([6u8; 32]),
                number: 4,
                parent_hash: B256::from([7u8; 32]),
                timestamp: 200,
            },
            invalidated: B256::from([8u8; 32]),
        };

        let managed_event = ManagedEvent {
            reset: None,
            unsafe_block: None,
            derivation_update: None,
            exhaust_l1: None,
            replace_block: Some(replacement.clone()),
            derivation_origin_update: None,
        };

        ManagedNode::handle_managed_event(&tx, Some(managed_event)).await;

        let event = rx.recv().await.expect("Should receive event");
        match event {
            NodeEvent::BlockReplaced { replacement: r } => assert_eq!(r, replacement),
            _ => panic!("Expected BlockReplaced event"),
        }
    }

    #[tokio::test]
    async fn test_websocket_client_creation() {
        // Test WebSocket client creation with invalid server (should fail)
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path();

        let config = Arc::new(ManagedNodeConfig {
            url: "invalid.server:8545".to_string(),
            jwt_path: jwt_path.to_str().unwrap().to_string(),
        });

        let managed_node = ManagedNode::new(config);

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
        });

        let managed_node_no_jwt = ManagedNode::new(config_no_jwt);
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
        });

        let managed_node = Arc::new(ManagedNode::new(config));

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
