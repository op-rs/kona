use super::ManagedEventTaskError;
use crate::syncnode::NodeEvent;
use alloy_eips::BlockNumberOrTag;
use alloy_network::Ethereum;
use alloy_provider::{Provider, RootProvider};
use jsonrpsee::ws_client::WsClient;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_rpc::ManagedModeApiClient;
use kona_supervisor_types::ManagedEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// [`ManagedEventTask`] sorts and processes individual events coming from a subscription.
#[derive(Debug)]
pub struct ManagedEventTask {
    /// The URL of the L1 RPC endpoint to use for fetching L1 data
    l1_rpc_url: String,
    /// The channel to send the events to which require further processing e.g. db updates
    event_tx: mpsc::Sender<NodeEvent>,
    /// The WebSocket client to use for connecting to managed node (optional for testing)
    client: Option<Arc<WsClient>>,
}

impl ManagedEventTask {
    /// Creates a new [`ManagedEventTask`] instance.
    pub const fn new(
        l1_rpc_url: String,
        event_tx: mpsc::Sender<NodeEvent>,
        client: Arc<WsClient>,
    ) -> Self {
        Self { l1_rpc_url, event_tx, client: Some(client) }
    }

    /// Processes a managed event received from the subscription.
    ///
    /// Analyzes the event content and takes appropriate actions based on the
    /// event fields.
    pub async fn handle_managed_event(&self, incoming_event: Option<ManagedEvent>) {
        match incoming_event {
            Some(event) => {
                debug!(target: "managed_event_task", %event, "Handling ManagedEvent");

                // Process each field of the event if it's present
                if let Some(reset_id) = &event.reset {
                    info!(target: "managed_event_task", %reset_id, "Reset event received");
                    // TODO: Handle reset action
                }

                if let Some(unsafe_block) = &event.unsafe_block {
                    info!(target: "managed_event_task", %unsafe_block, "Unsafe block event received");

                    // todo: check any pre processing needed
                    if let Err(err) =
                        self.event_tx.send(NodeEvent::UnsafeBlock { block: *unsafe_block }).await
                    {
                        warn!(target: "managed_event_task", %err, "Failed to send unsafe block event, channel closed or receiver dropped");
                    }
                }

                if let Some(derived_ref_pair) = &event.derivation_update {
                    info!(target: "managed_event_task", %derived_ref_pair, "Derivation update received");

                    // todo: check any pre processing needed
                    if let Err(err) = self
                        .event_tx
                        .send(NodeEvent::DerivedBlock {
                            derived_ref_pair: derived_ref_pair.clone(),
                        })
                        .await
                    {
                        warn!(target: "managed_event_task", %err, "Failed to derivation update event, channel closed or receiver dropped");
                    }
                }

                if let Some(derived_ref_pair) = &event.exhaust_l1 {
                    info!(target: "managed_event_task", ?derived_ref_pair, "L1 exhausted event received");

                    let provider =
                        RootProvider::<Ethereum>::new_http(self.l1_rpc_url.parse().unwrap());

                    if let Err(err) = self.handle_exhaust_l1(provider, derived_ref_pair).await {
                        error!(target: "managed_event_task", %err, "Failed to fetch next L1 block");
                    }
                }

                if let Some(replacement) = &event.replace_block {
                    info!(target: "managed_event_task", %replacement, "Block replacement received");

                    // todo: check any pre processing needed
                    if let Err(err) = self
                        .event_tx
                        .send(NodeEvent::BlockReplaced { replacement: replacement.clone() })
                        .await
                    {
                        warn!(target: "managed_event_task", %err, "Failed to send block replacement event, channel closed or receiver dropped");
                    }
                }

                if let Some(origin) = &event.derivation_origin_update {
                    info!(target: "managed_event_task", %origin, "Derivation origin update received");

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
                    debug!(target: "managed_event_task", "Received empty event with all fields None");
                }
            }
            None => {
                warn!(
                    target: "managed_event_task",
                    "Received None event, possibly an empty notification or an issue with deserialization."
                );
            }
        }
    }

    /// Handles the exhaust L1 event by fetching the next L1 block and providing it to the managed
    /// node.
    async fn handle_exhaust_l1(
        &self,
        provider: RootProvider,
        derived_ref_pair: &DerivedRefPair,
    ) -> Result<(), ManagedEventTaskError> {
        let next_block = provider
            .get_block_by_number(BlockNumberOrTag::Number(derived_ref_pair.source.number + 1))
            .await;
        match next_block {
            Ok(Some(block)) => {
                if block.header.parent_hash != derived_ref_pair.source.hash {
                    error!(target: "managed_event_task", "Block parent hash mismatch");
                    Err(ManagedEventTaskError::BlockHashMismatch {
                        current: derived_ref_pair.source.hash,
                        parent: block.header.parent_hash,
                    })?
                }

                let block_info = BlockInfo {
                    hash: block.header.hash,
                    number: block.header.number,
                    parent_hash: block.header.parent_hash,
                    timestamp: block.header.timestamp,
                };

                let client =
                    self.client.clone().ok_or(ManagedEventTaskError::ManagedNodeClientMissing)?;

                if let Err(err) =
                    ManagedModeApiClient::provide_l1(client.as_ref(), block_info).await
                {
                    error!(target: "managed_event_task", %err, "Error sending provide_l1 to managed node");
                    Err(ManagedEventTaskError::ManagedNodeAPICallFailed)?
                }

                info!(target: "managed_event_task", "Sent next L1 block to managed node using provide_l1");
                Ok(())
            }
            Ok(None) => {
                error!(target: "managed_event_task", "Next block is either empty or unavailable");
                Err(ManagedEventTaskError::NextBlockNotFound(derived_ref_pair.source.number + 1))?
            }
            Err(err) => {
                error!(target: "managed_event_task", %err, "Error fetching next L1 block");
                Err(ManagedEventTaskError::GetBlockByNumberFailed(
                    derived_ref_pair.source.number + 1,
                ))?
            }
        }
    }

    /// Creates a new [`ManagedEventTask`] instance for testing without a WebSocket client.
    #[cfg(test)]
    const fn new_for_testing(l1_rpc_url: String, event_tx: mpsc::Sender<NodeEvent>) -> Self {
        Self { l1_rpc_url, event_tx, client: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use alloy_transport::mock::*;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_types::BlockReplacement;

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

        let task = ManagedEventTask::new_for_testing("".to_string(), tx);

        task.handle_managed_event(Some(managed_event)).await;

        let event = rx.recv().await.expect("Should receive event");
        match event {
            NodeEvent::UnsafeBlock { block } => assert_eq!(block, block_info),
            _ => panic!("Expected UnsafeBlock event"),
        }
    }

    #[tokio::test]
    async fn test_handle_managed_event_sends_derivation_update() {
        let (tx, mut rx) = mpsc::channel(1);

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

        let task = ManagedEventTask::new_for_testing("".to_string(), tx);

        task.handle_managed_event(Some(managed_event)).await;

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
        let (tx, mut rx) = mpsc::channel(1);

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

        let task = ManagedEventTask::new_for_testing("".to_string(), tx);
        task.handle_managed_event(Some(managed_event)).await;

        let event = rx.recv().await.expect("Should receive event");
        match event {
            NodeEvent::BlockReplaced { replacement: r } => assert_eq!(r, replacement),
            _ => panic!("Expected BlockReplaced event"),
        }
    }

    #[tokio::test]
    async fn test_handle_managed_event_sends_exhaust_l1() {
        let (tx, _rx) = mpsc::channel(1);

        let derived_ref_pair = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([10u8; 32]),
                number: 5,
                parent_hash: B256::from([14u8; 32]),
                timestamp: 300,
            },
            derived: BlockInfo {
                hash: B256::from([11u8; 32]),
                number: 40,
                parent_hash: B256::from([12u8; 32]),
                timestamp: 301,
            },
        };

        let next_block = r#"{
            "number": "6",
            "hash": "0xd5f1812548be429cbdc6376b29611fc49e06f1359758c4ceaaa3b393e2239f9c",
            "mixHash": "0x24900fb3da77674a861c428429dce0762707ecb6052325bbd9b3c64e74b5af9d",
            "parentHash": "0x1f68ac259155e2f38211ddad0f0a15394d55417b185a93923e2abe71bb7a4d6d",
            "nonce": "0x378da40ff335b070",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "logsBloom": "0x00000000000000100000004080000000000500000000000000020000100000000800001000000004000001000000000000000800040010000020100000000400000010000000000000000040000000000000040000000000000000000000000000000400002400000000000000000000000000000004000004000000000000840000000800000080010004000000001000000800000000000000000000000000000000000800000000000040000000020000000000000000000800000400000000000000000000000600000400000000002000000000000000000000004000000000000000100000000000000000000000000000000000040000900010000000",
            "transactionsRoot": "0x4d0c8e91e16bdff538c03211c5c73632ed054d00a7e210c0eb25146c20048126",
            "stateRoot": "0x91309efa7e42c1f137f31fe9edbe88ae087e6620d0d59031324da3e2f4f93233",
            "receiptsRoot": "0x68461ab700003503a305083630a8fb8d14927238f0bc8b6b3d246c0c64f21f4a",
            "miner": "0xb42b6c4a95406c78ff892d270ad20b22642e102d",
            "difficulty": "0x66e619a",
            "totalDifficulty": "0x1e875d746ae",
            "extraData": "0xd583010502846765746885676f312e37856c696e7578",
            "size": "0x334",
            "gasLimit": "0x47e7c4",
            "gasUsed": "0x37993",
            "timestamp": "0x5835c54d",
            "uncles": [],
            "transactions": [
                "0xa0807e117a8dd124ab949f460f08c36c72b710188f01609595223b325e58e0fc",
                "0xeae6d797af50cb62a596ec3939114d63967c374fa57de9bc0f4e2b576ed6639d"
            ],
            "baseFeePerGas": "0x7",
            "withdrawalsRoot": "0x7a4ecf19774d15cf9c15adf0dd8e8a250c128b26c9e2ab2a08d6c9c8ffbd104f",
            "withdrawals": [],
            "blobGasUsed": "0x0",
            "excessBlobGas": "0x0",
            "parentBeaconBlockRoot": "0x95c4dbd5b19f6fe3cbc3183be85ff4e85ebe75c5b4fc911f1c91e5b7a554a685"
        }"#;

        let task = ManagedEventTask::new_for_testing("test.server".to_string(), tx);
        // Use mock provider to test exhaust_l1
        let asserter = Asserter::new();
        let provider = RootProvider::<Ethereum>::builder().connect_mocked_client(asserter.clone());

        // push the value that we expect on next call
        asserter.push(MockResponse::Success(serde_json::from_str(next_block).unwrap()));

        let result = task.handle_exhaust_l1(provider, &derived_ref_pair).await;

        assert!(result.is_err(), "Expected error");
        assert_eq!(
            result.err().unwrap(),
            ManagedEventTaskError::BlockHashMismatch {
                current: derived_ref_pair.source.hash,
                parent: "0x1f68ac259155e2f38211ddad0f0a15394d55417b185a93923e2abe71bb7a4d6d"
                    .parse()
                    .unwrap(),
            }
        );
    }
}
