//! [`NodeSubscriber`] implementation for subscribing to the events from managed node.

use crate::types::ManagedEvent;
use std::path::PathBuf;
use std::sync::Arc;
use alloy_primitives::U256;
use jsonrpsee::core::client::{Subscription, SubscriptionClientT};
use jsonrpsee::core::middleware::RpcServiceBuilder;
use jsonrpsee::ws_client::{WsClientBuilder, HeaderMap, HeaderValue};
use jsonrpsee::rpc_params;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, error};

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
    pub subscription: Option<Subscription<Option<ManagedEvent>>>
}

///
#[derive(Debug)]
pub struct NodeSubscriber {
    ///
    config: Arc<ManagedNodeConfig>,
    stop_tx: Option<watch::Sender<bool>>,
    task_handle: Option<JoinHandle<()>>,
}

// Implement the trait with matching error bounds

impl NodeSubscriber {
    ///
    pub fn new(config: Arc<ManagedNodeConfig>) -> Self {
        Self {
            config,
            stop_tx: None,
            task_handle: None,
        }
    }

    ///
    pub fn handle_managed_event(event_result: Option<ManagedEvent>) {
        // TODO: Call relevant DB functions to update the state
        match event_result {
            Some(event) => {
                info!("Handling ManagedEvent: {:?}", event);
                if event.reset.is_empty() {
                    info!("Reset is empty");
                } else {
                    info!("Reset: {}", event.reset);
                }
                if event.unsafe_block.is_none() {
                    info!("Unsafe block is None");
                } else {
                    info!("Unsafe block: {:?}", event.unsafe_block);
                }
                if event.derivation_update.is_none() {
                    info!("Derivation update is None");
                } else {
                    info!("Derivation update: {:?}", event.derivation_update);
                }
                if event.exhaust_l1.is_none() {
                    info!("Exhaust L1 is None");
                } else {
                    info!("Exhaust L1: {:?}", event.exhaust_l1);
                }
                if event.replace_block.is_none() {
                    info!("Replace block is None");
                } else {
                    info!("Replace block: {:?}", event.replace_block);
                }
                if event.derivation_origin_update.is_none() {
                    info!("Derivation origin update is None");
                } else {
                    info!("Derivation origin update: {:?}", event.derivation_origin_update);
                }
            }
            None => {
                info!("Received None event, possibly an empty notification or an issue with deserialization.");
            }
        }
    }

    ///
    pub async fn start_subscription(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.task_handle.is_some() {
            return Err("Subscription already active".into());
        }

        // Simplify the tracing initialization to avoid feature dependencies
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init()
            .ok(); // Allow re-initialization, though it might be better to initialize once globally

        let path = self.config.jwt_path.as_ref().ok_or_else(|| {
            error!(
                "No JWT token path provided for managed node. Please provide a path to the JWT token.",
            )
        });

        let jwt_secret = std::fs::read_to_string(path.unwrap())?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", jwt_secret))?,
        );

        let ws_url = format!("ws://{}:{}", self.config.url, self.config.port);
        let client = WsClientBuilder::default()
            .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(1024))
            .set_headers(headers)
            .build(&ws_url)
            .await?;

        // TODO: Cross check the subscription method name and params with Go implementation
        let mut subscription: Subscription<Option<ManagedEvent>> = client
            .subscribe("interop", rpc_params!["events"], "Unsubscribe")
            .await?;

        let (stop_tx, mut stop_rx) = watch::channel(false);
        self.stop_tx = Some(stop_tx);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    event_option = subscription.next() => {
                        match event_option {
                            Some(event_result) => {
                                info!("Received event: {:?}", event_result);
                                Self::handle_managed_event(event_result.unwrap_or_else(|e| {
                                    error!("Error in event deserialization: {:?}", e);
                                    None
                                }));
                            }
                            None => {
                                // Subscription closed by the server
                                info!("Subscription closed by server.");
                                break;
                            }
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            info!("Stop signal received, terminating subscription loop.");
                            break;
                        }
                    }
                }
            }
            // Attempt to gracefully unsubscribe, though the connection might already be closed.
            // This method might not exist or might have a different name depending on jsonrpsee version
            // and how subscriptions are managed. For now, we'll assume the loop breaking is enough.
            let _ = subscription.unsubscribe().await;
            info!("Subscription task finished.");
        });
        self.task_handle = Some(handle);
        Ok(())
    }

    ///
    pub async fn stop_subscription(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(stop_tx) = self.stop_tx.take() {
            stop_tx.send(true).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }

        if let Some(handle) = self.task_handle.take() {
            handle.await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            info!("Subscription stopped and task joined.");
        } else {
            return Err("Subscription not active or already stopped".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    use crate::types::{
        BlockRef,
        DerivedBlockRefPair,
        BlockReplacement
    };
    use alloy_primitives::{B256, U64};
    use serde_json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Move the test-specific implementation outside of the test function
    impl NodeSubscriber {
        #[cfg(test)]
        async fn start_test_subscription(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if self.task_handle.is_some() {
                return Err("Subscription already active".into());
            }

            let (stop_tx, mut stop_rx) = watch::channel(false);
            self.stop_tx = Some(stop_tx);

            // Create a simple task that just logs events and waits for stop signal
            let handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            info!("Test subscription tick");
                            
                            // Simulate receiving an event
                            let event = ManagedEvent {
                                reset: "test_reset".to_string(),
                                unsafe_block: None,
                                derivation_update: None,
                                exhaust_l1: None,
                                replace_block: None,
                                derivation_origin_update: None,
                            };
                            Self::handle_managed_event(Some(event));
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
            reset: "reset_id".to_string(),
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
                }
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
                timestamp: U64::from(1678886400)
            }),
        };

        let serialized = serde_json::to_string(&event).expect("Failed to serialize ManagedEvent");
        let deserialized: ManagedEvent = serde_json::from_str(&serialized).expect("Failed to deserialize ManagedEvent");

        assert_eq!(event, deserialized, "Serialized and deserialized events do not match");

        // Test with optional fields as None
        let event_with_nones = ManagedEvent {
            reset: "reset_id_nones".to_string(),
            unsafe_block: None,
            derivation_update: None,
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };

        let serialized_nones = serde_json::to_string(&event_with_nones).expect("Failed to serialize ManagedEvent with Nones");
        let deserialized_nones: ManagedEvent = serde_json::from_str(&serialized_nones).expect("Failed to deserialize ManagedEvent with Nones");

        assert_eq!(event_with_nones, deserialized_nones, "Serialized and deserialized events with Nones do not match");
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
        assert!(start_result.is_ok(), "Failed to start test subscription: {:?}", start_result.err());
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
            reset: "full_reset".to_string(),
            unsafe_block: Some(BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO }),
            derivation_update: Some(DerivedBlockRefPair {
                source: BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO },
                derived: BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO },
            }),
            exhaust_l1: Some(DerivedBlockRefPair {
                source: BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO },
                derived: BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO },
            }),
            replace_block: Some(BlockReplacement {
                replacement: BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO },
                invalidated: B256::ZERO,
            }),
            derivation_origin_update: Some(BlockRef { hash: B256::ZERO, number: U64::ZERO, parent_hash: B256::ZERO, timestamp: U64::ZERO }),
        };
        NodeSubscriber::handle_managed_event(Some(full_event));

        // Test with reset empty and all other optionals None
        let partial_event = ManagedEvent {
            reset: "".to_string(),
            unsafe_block: None,
            derivation_update: None,
            exhaust_l1: None,
            replace_block: None,
            derivation_origin_update: None,
        };
        NodeSubscriber::handle_managed_event(Some(partial_event));

        // Test with None event
        NodeSubscriber::handle_managed_event(None);
        // Assertions here would typically involve checking logs, which is hard in automated tests without a more complex setup.
        // For now, this test ensures the function runs without panicking for different inputs.
    }
}

