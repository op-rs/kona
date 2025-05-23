//! [`NodeSubscriber`] implementation for subscribing to the events from managed node.

use crate::{syncnode::SubscriptionError, types::ManagedEvent};
use alloy_primitives::U256;
use jsonrpsee::{
    core::{
        client::{Subscription, SubscriptionClientT},
        middleware::RpcServiceBuilder,
    },
    rpc_params,
    ws_client::{HeaderMap, HeaderValue, WsClientBuilder},
};
use std::{path::PathBuf, sync::Arc};
use tokio::{sync::watch, task::JoinHandle};
use tracing::{debug, error, info, warn};

/// Configuration for the managed node.
#[derive(Debug)]
pub struct ManagedNodeConfig {
    /// The chain ID of the L2
    pub chain_id: U256,
    /// The URL of the managed node
    pub url: String,
    /// The port of the managed node
    pub port: u16,
    /// The path to the JWT token for the managed node
    pub jwt_path: Option<PathBuf>,
    /// The current subscription to the managed node, if any
    pub subscription: Option<Subscription<Option<ManagedEvent>>>,
}

/// NodeSubscriber handles the subscription to managed node events.
///
/// It manages the WebSocket connection lifecycle and processes incoming events.
#[derive(Debug)]
pub struct NodeSubscriber {
    /// Configuration for connecting to the managed node
    config: Arc<ManagedNodeConfig>,
    /// Channel for signaling the subscription task to stop
    stop_tx: Option<watch::Sender<bool>>,
    /// Handle to the async subscription task
    task_handle: Option<JoinHandle<()>>,
}

impl NodeSubscriber {
    /// Creates a new NodeSubscriber with the specified configuration.
    pub const fn new(config: Arc<ManagedNodeConfig>) -> Self {
        Self { config, stop_tx: None, task_handle: None }
    }

    /// Processes a managed event received from the subscription.
    ///
    /// Analyzes the event content and takes appropriate actions based on the
    /// event fields. In the future, this will update the database with the
    /// event information.
    pub fn handle_managed_event(event_result: Option<ManagedEvent>) {
        // TODO: Call relevant DB functions to update the state
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
            return Err(SubscriptionError::AttachNodeError(
                "Subscription already active".to_string(),
            ));
        }

        // Initialize tracing
        tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).try_init().ok(); // Allow re-initialization

        // Get JWT token path
        let path = self.config.jwt_path.as_ref().ok_or_else(|| {
            let err_msg = "No JWT token path provided for managed node";
            error!("{}", err_msg);
            SubscriptionError::JwtError(err_msg.to_string())
        })?;

        // Read JWT secret
        let jwt_secret = std::fs::read_to_string(path).map_err(|e| {
            let err_msg = format!("Failed to read JWT token: {}", e);
            error!("{}", err_msg);
            SubscriptionError::JwtError(err_msg)
        })?;

        // Prepare headers with authorization
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", jwt_secret)).map_err(|e| {
                let err_msg = format!("Failed to create authorization header: {}", e);
                error!("{}", err_msg);
                SubscriptionError::Auth(err_msg)
            })?,
        );

        // Connect to WebSocket
        let ws_url = format!("ws://{}:{}", self.config.url, self.config.port);
        info!("Connecting to WebSocket at {}", ws_url);

        let client = WsClientBuilder::default()
            .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(1024))
            .set_headers(headers)
            .build(&ws_url)
            .await
            .map_err(|e| {
                let err_msg = format!("Failed to establish WebSocket connection: {}", e);
                error!("{}", err_msg);
                SubscriptionError::PubSub(err_msg)
            })?;

        // Subscribe to events
        // TODO: Cross check the subscription method name and params with Go implementation
        info!("Subscribing to interop events");
        let mut subscription: Subscription<Option<ManagedEvent>> = client
            .subscribe("interop", rpc_params!["events"], "Unsubscribe")
            .await
            .map_err(|e| {
                let err_msg = format!("Failed to subscribe to events: {}", e);
                error!("{}", err_msg);
                SubscriptionError::PubSub(err_msg)
            })?;

        // Setup stop channel
        let (stop_tx, mut stop_rx) = watch::channel(false);
        self.stop_tx = Some(stop_tx);

        // Start background task to handle events
        let handle = tokio::spawn(async move {
            info!("Subscription task started");

            loop {
                tokio::select! {
                    event_option = subscription.next() => {
                        match event_option {
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
                                warn!("Subscription closed by server");
                                break;
                            }
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            info!("Stop signal received, terminating subscription loop");
                            break;
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
        // Send stop signal
        if let Some(stop_tx) = self.stop_tx.take() {
            debug!("Sending stop signal to subscription task");
            stop_tx.send(true).map_err(|e| {
                let err_msg = format!("Failed to send stop signal: {:?}", e);
                error!("{}", err_msg);
                SubscriptionError::AttachNodeError(err_msg)
            })?;
        } else {
            return Err(SubscriptionError::AttachNodeError("No active stop channel".to_string()));
        }

        // Wait for task to complete
        if let Some(handle) = self.task_handle.take() {
            debug!("Waiting for subscription task to complete");
            handle.await.map_err(|e| {
                let err_msg = format!("Failed to join task: {:?}", e);
                error!("{}", err_msg);
                SubscriptionError::AttachNodeError(err_msg)
            })?;
            info!("Subscription stopped and task joined");
        } else {
            return Err(SubscriptionError::AttachNodeError(
                "Subscription not active or already stopped".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BlockRef, BlockReplacement, DerivedBlockRefPair};
    use alloy_primitives::{B256, U64};
    use serde_json;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tokio::time::{Duration, sleep};

    // Test-specific implementation
    impl NodeSubscriber {
        #[cfg(test)]
        async fn start_test_subscription(&mut self) -> Result<(), SubscriptionError> {
            if self.task_handle.is_some() {
                return Err(SubscriptionError::AttachNodeError(
                    "Subscription already active".to_string(),
                ));
            }

            let (stop_tx, mut stop_rx) = watch::channel(false);
            self.stop_tx = Some(stop_tx);

            // Create a simple task that simulates events and waits for stop signal
            let handle = tokio::spawn(async move {
                info!("Test subscription task started");

                // Simulate different types of events
                let events = vec![
                    // Empty event
                    ManagedEvent::default(),
                    // Reset event
                    ManagedEvent {
                        reset: Some("test_reset_123".to_string()),
                        ..ManagedEvent::default()
                    },
                    // Unsafe block event
                    ManagedEvent {
                        unsafe_block: Some(BlockRef {
                            hash: B256::from_slice(&[1; 32]),
                            number: U64::from(100),
                            parent_hash: B256::from_slice(&[2; 32]),
                            timestamp: U64::from(1678886400),
                        }),
                        ..ManagedEvent::default()
                    },
                ];

                let mut index = 0;
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            info!("Test subscription tick");

                            // Simulate receiving an event, cycling through the test events
                            let event = events[index % events.len()].clone();
                            Self::handle_managed_event(Some(event));
                            index += 1;
                        }
                        _ = stop_rx.changed() => {
                            if *stop_rx.borrow() {
                                info!("Test subscription stopped");
                                break;
                            }
                        }
                    }
                }
                info!("Test subscription task finished");
            });

            self.task_handle = Some(handle);
            Ok(())
        }
    }

    fn create_mock_jwt_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, "testjwtsecret").expect("Failed to write to temp file");
        file
    }

    #[tokio::test]
    async fn test_managed_event_serialization_deserialization() {
        let event = ManagedEvent {
            reset: Some("reset_id".to_string()),
            unsafe_block: Some(BlockRef {
                hash: B256::from_slice(&[1; 32]),
                number: U64::from(124),
                parent_hash: B256::from_slice(&[2; 32]),
                timestamp: U64::from(1678886400),
            }),
            derivation_update: Some(DerivedBlockRefPair {
                source: BlockRef {
                    hash: B256::from_slice(&[3; 32]),
                    number: U64::from(124),
                    parent_hash: B256::from_slice(&[4; 32]),
                    timestamp: U64::from(1678886400),
                },
                derived: BlockRef {
                    hash: B256::from_slice(&[5; 32]),
                    number: U64::from(124),
                    parent_hash: B256::from_slice(&[6; 32]),
                    timestamp: U64::from(1678886400),
                },
            }),
            exhaust_l1: Some(DerivedBlockRefPair {
                source: BlockRef {
                    hash: B256::from_slice(&[7; 32]),
                    number: U64::from(124),
                    parent_hash: B256::from_slice(&[8; 32]),
                    timestamp: U64::from(1678886400),
                },
                derived: BlockRef {
                    hash: B256::from_slice(&[9; 32]),
                    number: U64::from(124),
                    parent_hash: B256::from_slice(&[10; 32]),
                    timestamp: U64::from(1678886400),
                },
            }),
            replace_block: Some(BlockReplacement {
                replacement: BlockRef {
                    hash: B256::from_slice(&[11; 32]),
                    number: U64::from(124),
                    parent_hash: B256::from_slice(&[12; 32]),
                    timestamp: U64::from(1678886400),
                },
                invalidated: B256::from_slice(&[13; 32]),
            }),
            derivation_origin_update: Some(BlockRef {
                hash: B256::from_slice(&[14; 32]),
                number: U64::from(50),
                parent_hash: B256::from_slice(&[15; 32]),
                timestamp: U64::from(1678886400),
            }),
        };

        let serialized = serde_json::to_string(&event).expect("Failed to serialize ManagedEvent");
        let deserialized: ManagedEvent =
            serde_json::from_str(&serialized).expect("Failed to deserialize ManagedEvent");

        assert_eq!(event, deserialized, "Serialized and deserialized events do not match");

        // Test with optional fields as None
        let event_with_nones = ManagedEvent {
            reset: None,
            unsafe_block: None,
            derivation_update: None,
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };

        let serialized_nones = serde_json::to_string(&event_with_nones)
            .expect("Failed to serialize ManagedEvent with Nones");
        let deserialized_nones: ManagedEvent = serde_json::from_str(&serialized_nones)
            .expect("Failed to deserialize ManagedEvent with Nones");

        assert_eq!(
            event_with_nones, deserialized_nones,
            "Serialized and deserialized events with Nones do not match"
        );
    }

    #[tokio::test]
    async fn test_node_subscriber_start_stop() {
        let jwt_file = create_mock_jwt_file();
        let jwt_path = jwt_file.path().to_path_buf();

        // Create a node config without an actual server
        let config = Arc::new(ManagedNodeConfig {
            chain_id: U256::from(1),
            url: "mock.server".to_string(),
            port: 8545,
            jwt_path: Some(jwt_path),
            subscription: None,
        });

        let mut subscriber = NodeSubscriber::new(config);

        // Use the test subscription instead of the real one
        let start_result = subscriber.start_test_subscription().await;
        assert!(
            start_result.is_ok(),
            "Failed to start test subscription: {:?}",
            start_result.err()
        );
        assert!(subscriber.task_handle.is_some(), "Task handle should exist after starting");

        // Allow some time for the subscription task to process
        sleep(Duration::from_millis(300)).await;

        // Test stopping the subscription
        let stop_result = subscriber.stop_subscription().await;
        assert!(stop_result.is_ok(), "Failed to stop subscription: {:?}", stop_result.err());
        assert!(subscriber.task_handle.is_none(), "Task handle should not exist after stopping");
        assert!(subscriber.stop_tx.is_none(), "Stop_tx should not exist after stopping");
    }

    #[tokio::test]
    async fn test_handle_managed_event_logging() {
        // Test with all fields Some
        let full_event = ManagedEvent {
            reset: Some("full_reset".to_string()),
            unsafe_block: Some(BlockRef {
                hash: B256::ZERO,
                number: U64::ZERO,
                parent_hash: B256::ZERO,
                timestamp: U64::ZERO,
            }),
            derivation_update: Some(DerivedBlockRefPair {
                source: BlockRef {
                    hash: B256::ZERO,
                    number: U64::ZERO,
                    parent_hash: B256::ZERO,
                    timestamp: U64::ZERO,
                },
                derived: BlockRef {
                    hash: B256::ZERO,
                    number: U64::ZERO,
                    parent_hash: B256::ZERO,
                    timestamp: U64::ZERO,
                },
            }),
            exhaust_l1: Some(DerivedBlockRefPair {
                source: BlockRef {
                    hash: B256::ZERO,
                    number: U64::ZERO,
                    parent_hash: B256::ZERO,
                    timestamp: U64::ZERO,
                },
                derived: BlockRef {
                    hash: B256::ZERO,
                    number: U64::ZERO,
                    parent_hash: B256::ZERO,
                    timestamp: U64::ZERO,
                },
            }),
            replace_block: Some(BlockReplacement {
                replacement: BlockRef {
                    hash: B256::ZERO,
                    number: U64::ZERO,
                    parent_hash: B256::ZERO,
                    timestamp: U64::ZERO,
                },
                invalidated: B256::ZERO,
            }),
            derivation_origin_update: Some(BlockRef {
                hash: B256::ZERO,
                number: U64::ZERO,
                parent_hash: B256::ZERO,
                timestamp: U64::ZERO,
            }),
        };
        NodeSubscriber::handle_managed_event(Some(full_event));

        // Test with reset empty and all other optionals None
        let partial_event = ManagedEvent {
            reset: None,
            unsafe_block: None,
            derivation_update: None,
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };
        NodeSubscriber::handle_managed_event(Some(partial_event));

        // Test with None event
        NodeSubscriber::handle_managed_event(None);
        // Assertions here would typically involve checking logs, which is hard in automated tests
        // without a more complex setup. For now, this test ensures the function runs
        // without panicking for different inputs.
    }
}
