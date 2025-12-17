//! [`NodeActor`] implementation for a follow client that periodically polls another L2 CL node's
//! sync status.

use crate::{
    FollowClient, FollowStatus, NodeActor,
    actors::{CancellableContext, follow::error::FollowActorError},
};
use async_trait::async_trait;
use std::time::Duration;
use tokio::{
    select,
    sync::mpsc,
    time::interval,
};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// Default polling interval in seconds for querying the follow source.
const DEFAULT_FOLLOW_POLL_INTERVAL: u64 = 2;

/// An actor that periodically polls a follow client for sync status updates.
///
/// The [`FollowActor`] queries another L2 consensus layer node's sync status at regular
/// intervals and sends the results to the engine actor.
#[derive(Debug)]
pub struct FollowActor<FC>
where
    FC: FollowClient,
{
    /// The follow client for querying sync status.
    follow_client: FC,
    /// Channel to send follow status updates to the engine.
    follow_status_tx: mpsc::Sender<FollowStatus>,
    /// The polling interval for querying the follow source.
    poll_interval: Duration,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl<FC> FollowActor<FC>
where
    FC: FollowClient,
{
    /// Creates a new [`FollowActor`] with the default polling interval.
    ///
    /// # Arguments
    ///
    /// * `follow_client` - The follow client for querying sync status
    /// * `follow_status_tx` - Channel to send follow status updates to the engine
    /// * `cancellation` - Cancellation token for graceful shutdown
    pub fn new(
        follow_client: FC,
        follow_status_tx: mpsc::Sender<FollowStatus>,
        cancellation: CancellationToken,
    ) -> Self {
        Self::new_with_interval(
            follow_client,
            follow_status_tx,
            cancellation,
            Duration::from_secs(DEFAULT_FOLLOW_POLL_INTERVAL),
        )
    }

    /// Creates a new [`FollowActor`] with a custom polling interval.
    ///
    /// # Arguments
    ///
    /// * `follow_client` - The follow client for querying sync status
    /// * `follow_status_tx` - Channel to send follow status updates to the engine
    /// * `cancellation` - Cancellation token for graceful shutdown
    /// * `poll_interval` - Custom polling interval
    pub const fn new_with_interval(
        follow_client: FC,
        follow_status_tx: mpsc::Sender<FollowStatus>,
        cancellation: CancellationToken,
        poll_interval: Duration,
    ) -> Self {
        Self { follow_client, follow_status_tx, poll_interval, cancellation }
    }

    /// Validates L1 block canonicality and sends the status to the engine if valid.
    ///
    /// Checks that the L1 blocks referenced in the follow status (external safe L1 origin,
    /// external finalized L1 origin, and current L1) are canonical on the L1 chain.
    ///
    /// # Arguments
    ///
    /// * `status` - The follow status to validate and send
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation passes and status is sent successfully, or an error
    /// if validation fails or the send fails.
    async fn validate_and_send(&self, status: FollowStatus) -> Result<(), FollowActorError> {
        // Validate external safe L1 origin
        if !self
            .validate_l1_block(status.safe_l2.l1_origin.number, status.safe_l2.l1_origin.hash)
            .await?
        {
            warn!(
                target: "follow_actor",
                l1_number = status.safe_l2.l1_origin.number,
                l1_hash = ?status.safe_l2.l1_origin.hash,
                "Invalid L1 origin for external safe block, dropping update"
            );
            return Ok(());
        }

        // Validate external finalized L1 origin
        if !self
            .validate_l1_block(
                status.finalized_l2.l1_origin.number,
                status.finalized_l2.l1_origin.hash,
            )
            .await?
        {
            warn!(
                target: "follow_actor",
                l1_number = status.finalized_l2.l1_origin.number,
                l1_hash = ?status.finalized_l2.l1_origin.hash,
                "Invalid L1 origin for external finalized block, dropping update"
            );
            return Ok(());
        }

        // Validate current L1 block
        if !self.validate_l1_block(status.current_l1.number, status.current_l1.hash).await? {
            warn!(
                target: "follow_actor",
                l1_number = status.current_l1.number,
                l1_hash = ?status.current_l1.hash,
                "Invalid current L1 block, dropping update"
            );
            return Ok(());
        }

        // All validations passed, send to engine
        self.follow_status_tx
            .send(status)
            .await
            .map_err(|e| FollowActorError::ChannelClosed(e.to_string()))?;

        Ok(())
    }

    /// Validates that an L1 block is canonical on the L1 chain.
    ///
    /// Fetches the canonical block at the given number and compares the hash.
    ///
    /// # Arguments
    ///
    /// * `number` - The L1 block number
    /// * `hash` - The expected L1 block hash
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the block is canonical, `Ok(false)` if it's not canonical,
    /// or an error if the L1 RPC call fails.
    async fn validate_l1_block(
        &self,
        number: u64,
        hash: alloy_primitives::B256,
    ) -> Result<bool, FollowActorError> {
        // Fetch the canonical block at this number from L1
        let canonical_block = self
            .follow_client
            .l1_block_info_by_number(number)
            .await
            .map_err(|e| FollowActorError::L1ValidationError(e.to_string()))?;

        // Compare hashes
        Ok(canonical_block.hash == hash)
    }
}

#[async_trait]
impl<FC> NodeActor for FollowActor<FC>
where
    FC: FollowClient + 'static,
{
    type Error = FollowActorError;
    type StartData = ();

    /// Start the main processing loop.
    ///
    /// Periodically polls the follow client for sync status and logs the results.
    /// Continues until cancellation is requested.
    async fn start(self, _: Self::StartData) -> Result<(), Self::Error> {
        let cancel = self.cancellation.clone();
        let mut ticker = interval(self.poll_interval);

        info!(
            target: "follow_actor",
            interval_secs = ?self.poll_interval.as_secs(),
            "Starting follow actor"
        );

        loop {
            select! {
                _ = cancel.cancelled() => {
                    info!(
                        target: "follow_actor",
                        "Received shutdown signal. Exiting follow actor task."
                    );
                    return Ok(());
                },
                _ = ticker.tick() => {
                    // Query the follow client for sync status
                    match self.follow_client.get_follow_status().await {
                        Ok(status) => {
                            info!(
                                target: "follow_actor",
                                current_l1_number = status.current_l1.number,
                                current_l1_hash = ?status.current_l1.hash,
                                safe_l2_number = status.safe_l2.block_info.number,
                                safe_l2_hash = ?status.safe_l2.block_info.hash,
                                finalized_l2_number = status.finalized_l2.block_info.number,
                                finalized_l2_hash = ?status.finalized_l2.block_info.hash,
                                "Received follow status update"
                            );

                            // Validate L1 block canonicality before sending to engine
                            if let Err(e) = self.validate_and_send(status).await {
                                warn!(
                                    target: "follow_actor",
                                    error = ?e,
                                    "Failed to validate or send follow status"
                                );
                            }
                        }
                        Err(e) => {
                            warn!(
                                target: "follow_actor",
                                error = ?e,
                                "Failed to get follow status, will retry on next interval"
                            );
                        }
                    }
                }
            }
        }
    }
}

impl<FC> CancellableContext for FollowActor<FC>
where
    FC: FollowClient,
{
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}
