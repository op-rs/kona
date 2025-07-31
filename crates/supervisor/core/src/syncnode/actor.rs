use crate::syncnode::{
    ManagedNodeClient, ManagedNodeCommand, ManagedNodeController, SubscriptionHandler,
    utils::spawn_task_with_retry,
};
use anyhow::Error;
use derive_more::Constructor;
use kona_interop::ManagedEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Actor for managing a node in the supervisor environment.
#[derive(Debug, Constructor)]
pub struct ManagedNodeActor<C, N> {
    client: Arc<C>,
    node: Arc<N>,
    cancel_token: CancellationToken,
}

impl<C, N> ManagedNodeActor<C, N>
where
    C: ManagedNodeClient + 'static,
    N: ManagedNodeController + SubscriptionHandler + 'static,
{
    /// Starts the managed node actor, processing commands from the provided channel.
    pub fn start(self, command_rx: mpsc::Receiver<ManagedNodeCommand>) {
        // Task 1: Command handling
        let node = self.node.clone();
        let cancel_token = self.cancel_token.clone();
        tokio::spawn(async move {
            run_command_task(node, command_rx, cancel_token).await;
        });

        // Task 2: Subscription handling
        spawn_task_with_retry(
            move || {
                let handler = self.node.clone();
                let client = self.client.clone();

                async move { run_subscription_task(client, handler).await }
            },
            self.cancel_token.clone(),
            usize::MAX,
        );
    }
}

async fn run_command_task<N>(
    node: Arc<N>,
    mut command_rx: mpsc::Receiver<ManagedNodeCommand>,
    cancel_token: CancellationToken,
) where
    N: ManagedNodeController + SubscriptionHandler + 'static,
{
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!(target: "supervisor::syncnode", "Cancellation requested, shutting down command task");
                break;
            }
            maybe_cmd = command_rx.recv() => {
                match maybe_cmd {
                    Some(cmd) => {
                        match cmd {
                            ManagedNodeCommand::UpdateFinalized { block_id } => {
                                let result = node.update_finalized(block_id).await;
                                if let Err(err) = result {
                                    warn!(
                                        target: "supervisor::syncnode",
                                        %err,
                                        "Failed to update finalized block"
                                    );
                                }
                            }
                            ManagedNodeCommand::UpdateCrossUnsafe { block_id } => {
                                let result = node.update_cross_unsafe(block_id).await;
                                if let Err(err) = result {
                                    warn!(
                                        target: "supervisor::syncnode",
                                        %err,
                                        "Failed to update cross unsafe block"
                                    );
                                }
                            }
                            ManagedNodeCommand::UpdateCrossSafe { source_block_id, derived_block_id } => {
                                let result = node.update_cross_safe(source_block_id, derived_block_id).await;
                                if let Err(err) = result {
                                    warn!(
                                        target: "supervisor::syncnode",
                                        %err,
                                        "Failed to update cross safe block"
                                    );
                                }
                            }
                            ManagedNodeCommand::Reset {} => {
                                let result = node.reset().await;
                                if let Err(err) = result {
                                    warn!(
                                        target: "supervisor::syncnode",
                                        %err,
                                        "Failed to reset managed node"
                                    );
                                }
                            }
                            ManagedNodeCommand::InvalidateBlock { seal } => {
                                let result = node.invalidate_block(seal).await;
                                if let Err(err) = result {
                                    warn!(
                                        target: "supervisor::syncnode",
                                        %err,
                                        "Failed to invalidate block"
                                    );
                                }
                            }
                        }
                    }
                    None => {
                        info!(target: "supervisor::syncnode", "Command channel closed, shutting down command task");
                        break;
                    }
                }
            }
        }
    }
}

async fn run_subscription_task<C: ManagedNodeClient, N: SubscriptionHandler>(
    client: Arc<C>,
    handler: Arc<N>,
) -> Result<(), Error> {
    let mut subscription = client.subscribe_events().await.inspect_err(|err| {
        error!(
            target: "supervisor::syncnode",
            %err,
            "Failed to subscribe to node events"
        );
    })?;

    info!(target: "supervisor::syncnode", "Subscription stream started");
    loop {
        tokio::select! {
            incoming_event = subscription.next() => {
                match incoming_event {
                    Some(Ok(subscription_event)) => {
                        if let Some(event) = subscription_event.data {
                            handle_subscription_event(&handler, event).await;
                        }
                    }
                    Some(Err(err)) => {
                        error!(
                            target: "supervisor::managed_event_task",
                            %err,
                            "Error in event deserialization"
                        );
                        return Err(err.into());
                    }
                    None => {
                        warn!(target: "supervisor::managed_event_task", "Subscription closed by server");
                        client.reset_ws_client().await;
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn handle_subscription_event<N: SubscriptionHandler>(handler: &Arc<N>, event: ManagedEvent) {
    if let Some(reset_id) = &event.reset {
        if let Err(err) = handler.handle_reset(reset_id).await {
            warn!(
              target: "supervisor::syncnode",
              %err,
              "Failed to handle reset event"
            );
        }
    }

    if let Some(unsafe_block) = &event.unsafe_block {
        if let Err(err) = handler.handle_unsafe_block(unsafe_block).await {
            warn!(
                target: "supervisor::syncnode",
                %err,
                "Failed to handle unsafe block event"
            );
        }
    }

    if let Some(derived_ref_pair) = &event.derivation_update {
        info!(
            target: "supervisor::syncnode",
            block_number = derived_ref_pair.derived.number,
            "Handling derivation update event"
        );
        if event.derivation_origin_update.is_none() {
            if let Err(err) = handler.handle_derivation_update(derived_ref_pair).await {
                warn!(
                    target: "supervisor::syncnode",
                    %err,
                    "Failed to handle derivation update event"
                );
            }
        }
    }

    if let Some(replacement) = &event.replace_block {
        if let Err(err) = handler.handle_replace_block(replacement).await {
            warn!(
                target: "supervisor::syncnode",
                %err,
                "Failed to handle block replacement event"
            );
        }
    }

    if let Some(origin) = &event.derivation_origin_update {
        if let Err(err) = handler.handle_derivation_origin_update(origin).await {
            warn!(
                target: "supervisor::syncnode",
                %err,
                "Failed to handle derivation origin update event"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::syncnode::{
        ClientError, ManagedNodeClient, ManagedNodeCommand, ManagedNodeController,
        ManagedNodeError, SubscriptionHandler,
    };
    use alloy_eips::BlockNumHash;
    use alloy_primitives::{B256, ChainId};
    use jsonrpsee::core::client::Subscription;
    use kona_interop::{BlockReplacement, DerivedRefPair};
    use kona_protocol::BlockInfo;
    use kona_supervisor_types::{BlockSeal, OutputV0, Receipts, SubscriptionEvent};
    use mockall::{mock, predicate::*};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    // Mock the ManagedNodeController trait
    mock! {
        #[derive(Debug)]
        pub Node {}

        #[async_trait::async_trait]
        impl ManagedNodeController for Node {
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_unsafe(&self, cross_unsafe_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_safe(&self,source_block_id: BlockNumHash,derived_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn reset(&self) -> Result<(), ManagedNodeError>;
            async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError>;
        }

        #[async_trait::async_trait]
        impl SubscriptionHandler for Node {
            async fn handle_exhaust_l1(&self, derived_ref_pair: DerivedRefPair) -> Result<(), ManagedNodeError>;
            async fn handle_reset(&self, reset_id: &String) -> Result<(), ManagedNodeError>;
            async fn handle_unsafe_block(&self, block: &BlockInfo) -> Result<(), ManagedNodeError>;
            async fn handle_derivation_update(&self, derived_ref_pair: &DerivedRefPair) -> Result<(), ManagedNodeError>;
            async fn handle_replace_block(&self, replacement: &BlockReplacement) -> Result<(), ManagedNodeError>;
            async fn handle_derivation_origin_update(&self, origin: &BlockInfo) -> Result<(), ManagedNodeError>;
        }
    }

    mock! {
        #[derive(Debug)]
        pub NodeClient {}

        #[async_trait::async_trait]
        impl ManagedNodeClient for NodeClient {
            async fn chain_id(&self) -> Result<ChainId, ClientError>;
            async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ClientError>;
            async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ClientError>;
            async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ClientError>;
            async fn pending_output_v0_at_timestamp(&self, timestamp: u64)-> Result<OutputV0, ClientError>;
            async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ClientError>;
            async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ClientError>;
            async fn reset_pre_interop(&self) -> Result<(), ClientError>;
            async fn reset(
                &self,
                unsafe_id: BlockNumHash,
                cross_unsafe_id: BlockNumHash,
                local_safe_id: BlockNumHash,
                cross_safe_id: BlockNumHash,
                finalised_id: BlockNumHash,
            ) -> Result<(), ClientError>;
            async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ClientError>;
            async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ClientError>;
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn update_cross_unsafe(&self,cross_unsafe_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn update_cross_safe(&self,source_block_id: BlockNumHash,derived_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn reset_ws_client(&self);
        }
    }

    #[tokio::test]
    async fn test_run_command_task_update_finalized_and_reset() {
        let mut mock_node = MockNode::new();
        mock_node.expect_update_finalized().times(1).returning(|_| Ok(()));
        mock_node.expect_reset().times(1).returning(|| Ok(()));

        let node = Arc::new(mock_node);
        let (tx, rx) = mpsc::channel(10);
        let cancel_token = CancellationToken::new();

        // Spawn the command task
        let handle = tokio::spawn(super::run_command_task(node.clone(), rx, cancel_token.clone()));

        // Send commands
        tx.send(ManagedNodeCommand::UpdateFinalized {
            block_id: BlockNumHash::new(1, B256::random()),
        })
        .await
        .unwrap();
        tx.send(ManagedNodeCommand::Reset {}).await.unwrap();

        // Drop the sender to close the channel and end the task
        drop(tx);

        // Wait for the task to finish
        handle.await.unwrap();
    }
}
