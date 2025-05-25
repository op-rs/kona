//! [`ManagedNodeSubscriber`] implementation for subscribing to the events from managed node.

use alloy_rpc_types_engine::JwtSecret;
use jsonrpsee::{
    core::{SubscriptionError, client::Subscription},
    ws_client::{HeaderMap, HeaderValue, WsClientBuilder},
};
use kona_supervisor_rpc::ManagedNodeApiClient;
use kona_supervisor_types::ManagedEvent;
use std::{fs::File, sync::Arc};
use tokio::{sync::watch, task::JoinHandle};
use tracing::{debug, error, info, warn};

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
    /// If the file is not found, it will create a random JWT and write it to the file.
    pub fn default_jwt_secret() -> Option<JwtSecret> {
        let cur_dir = std::env::current_dir().ok()?;
        std::fs::read_to_string(cur_dir.join("jwt.hex")).map_or_else(
            |_| {
                use std::io::Write;
                let secret = JwtSecret::random();
                if let Ok(mut file) = File::create("jwt.hex") {
                    if let Err(e) =
                        file.write_all(alloy_primitives::hex::encode(secret.as_bytes()).as_bytes())
                    {
                        tracing::error!("Failed to write JWT secret to file: {:?}", e);
                    }
                }
                Some(secret)
            },
            |content| JwtSecret::from_hex(content).ok(),
        )
    }
}

/// [`ManagedNodeSubscriber`] handles the subscription to managed node events.
///
/// It manages the WebSocket connection lifecycle and processes incoming events.
#[derive(Debug)]
pub struct ManagedNodeSubscriber {
    /// Configuration for connecting to the managed node
    config: Arc<ManagedNodeConfig>,
    /// Channel for signaling the subscription task to stop
    stop_tx: Option<watch::Sender<bool>>,
    /// Handle to the async subscription task
    task_handle: Option<JoinHandle<()>>,
}

impl ManagedNodeSubscriber {
    /// Creates a new [`ManagedNodeSubscriber`] with the specified configuration.
    pub const fn new(config: Arc<ManagedNodeConfig>) -> Self {
        Self { config, stop_tx: None, task_handle: None }
    }

    /// Processes a managed event received from the subscription.
    ///
    /// Analyzes the event content and takes appropriate actions based on the
    /// event fields.
    /// TODO: Call relevant DB functions to update the state
    pub fn handle_managed_event(event_result: Option<ManagedEvent>) {
        match event_result {
            Some(event) => {
                debug!("Handling ManagedEvent: {:?}", event);

                // Process each field of the event if it's present
                if let Some(reset_id) = &event.reset {
                    info!("Reset event received with ID: {}", reset_id);
                    // Handle reset action
                }

                if let Some(unsafe_block) = &event.unsafe_block {
                    info!(
                        "Unsafe block event received: hash={:?}, number={}",
                        unsafe_block.hash, unsafe_block.number
                    );
                    // Handle unsafe block
                }

                if let Some(update) = &event.derivation_update {
                    info!(
                        "Derivation update received: source={:?}, derived={:?}",
                        update.source.number, update.derived.number
                    );
                    // Handle derivation update
                }

                if let Some(exhaust) = &event.exhaust_l1 {
                    info!(
                        "L1 exhausted: source={:?}, derived={:?}",
                        exhaust.source.number, exhaust.derived.number
                    );
                    // Handle L1 exhaustion
                }

                if let Some(replacement) = &event.replace_block {
                    info!(
                        "Block replacement: new={:?}, invalidated={:?}",
                        replacement.replacement.hash, replacement.invalidated
                    );
                    // Handle block replacement
                }

                if let Some(origin) = &event.derivation_origin_update {
                    info!(
                        "Derivation origin updated: hash={:?}, number={}",
                        origin.hash, origin.number
                    );
                    // Handle derivation origin update
                }

                // Check if this was an empty event (all fields None)
                if event.reset.is_none() &&
                    event.unsafe_block.is_none() &&
                    event.derivation_update.is_none() &&
                    event.exhaust_l1.is_none() &&
                    event.replace_block.is_none() &&
                    event.derivation_origin_update.is_none()
                {
                    debug!("Received empty event with all fields None");
                }
            }
            None => {
                warn!(
                    "Received None event, possibly an empty notification or an issue with deserialization."
                );
            }
        }
    }

    /// Starts a subscription to the managed node.
    ///
    /// Establishes a WebSocket connection and subscribes to node events.
    /// Spawns a background task to process incoming events.
    pub async fn start_subscription(&mut self) -> Result<(), SubscriptionError> {
        if self.task_handle.is_some() {
            return Err(SubscriptionError::from("Subscription already active".to_string()));
        }

        let jwt_secret = self
            .config
            .jwt_secret()
            .ok_or_else(|| SubscriptionError::from("Incorrect JWT secret".to_string()))?;

        let mut headers = HeaderMap::new();
        let auth_header =
            format!("Bearer {}", alloy_primitives::hex::encode(jwt_secret.as_bytes()));
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&auth_header).map_err(|e| {
                SubscriptionError::from(format!("Invalid authorization header: {}", e))
            })?,
        );

        // Connect to WebSocket
        let ws_url = format!("ws://{}", self.config.url);
        info!("Connecting to WebSocket at {}", ws_url);

        let client =
            WsClientBuilder::default().set_headers(headers).build(&ws_url).await.map_err(|e| {
                let err_msg = format!("Failed to establish WebSocket connection: {}", e);
                error!("{}", err_msg);
                SubscriptionError::from(err_msg)
            })?;

        let mut subscription: Subscription<Option<ManagedEvent>> =
            ManagedNodeApiClient::subscribe_events(&client).await.map_err(|e| {
                let err_msg = format!("Failed to subscribe to events: {}", e);
                error!("{}", err_msg);
                SubscriptionError::from(err_msg)
            })?;

        // Start background task to handle events
        let handle = tokio::spawn(async move {
            info!("Subscription task started");
            loop {
                tokio::select! {
                    event = subscription.next() => {
                        match event {
                            Some(event_result) => {
                                debug!("Received event: {:?}", event_result);
                                match event_result {
                                    Ok(managed_event) => {
                                        Self::handle_managed_event(managed_event);
                                    },
                                    Err(e) => {
                                        error!("Error in event deserialization: {:?}", e);
                                        // Continue processing next events despite this error
                                    }
                                }
                            }
                            None => {
                                // Subscription closed by the server
                                warn!("No event received");
                                break;
                            }
                        }
                    }
                }
            }

            // Try to unsubscribe gracefully
            if let Err(e) = subscription.unsubscribe().await {
                warn!("Failed to unsubscribe gracefully: {:?}", e);
            }

            info!("Subscription task finished");
        });

        self.task_handle = Some(handle);
        info!("Subscription started successfully");
        Ok(())
    }

    /// Stops the subscription to the managed node.
    ///
    /// Sends a stop signal to the background task and waits for it to complete.
    pub async fn stop_subscription(&mut self) -> Result<(), SubscriptionError> {
        if let Some(stop_tx) = self.stop_tx.take() {
            debug!("Sending stop signal to subscription task");
            stop_tx.send(true).map_err(|e| {
                let err_msg = format!("Failed to send stop signal: {:?}", e);
                error!("{}", err_msg);
                SubscriptionError::from(err_msg)
            })?;
        } else {
            return Err(SubscriptionError::from("No active stop channel".to_string()));
        }

        // Wait for task to complete
        if let Some(handle) = self.task_handle.take() {
            debug!("Waiting for subscription task to complete");
            handle.await.map_err(|e| {
                let err_msg = format!("Failed to join task: {:?}", e);
                error!("{}", err_msg);
                SubscriptionError::from(err_msg)
            })?;
            info!("Subscription stopped and task joined");
        } else {
            return Err(SubscriptionError::from(
                "Subscription not active or already stopped".to_string(),
            ));
        }

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

        // Test with invalid path
        let config_invalid = ManagedNodeConfig {
            url: "test.server".to_string(),
            jwt_path: "/nonexistent/path/jwt.hex".to_string(),
        };

        let jwt_secret_fallback = config_invalid.jwt_secret();
        assert!(jwt_secret_fallback.is_some(), "Should fall back to default JWT secret creation");

        // Clean up the jwt.hex file created during test
        let _ = std::fs::remove_file("jwt.hex");
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

        let mut subscriber = ManagedNodeSubscriber::new(config);

        // Test that we can create the subscriber instance
        assert!(subscriber.task_handle.is_none());
        assert!(subscriber.stop_tx.is_none());

        // Test starting subscription to invalid server (should fail)
        let start_result = subscriber.start_subscription().await;
        assert!(start_result.is_err(), "Subscription to invalid server should fail");

        // Verify state remains consistent after failure
        assert!(subscriber.task_handle.is_none());
    }
}
