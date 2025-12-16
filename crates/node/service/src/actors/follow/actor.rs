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

                            // Send the status to the engine
                            if let Err(e) = self.follow_status_tx.send(status).await {
                                error!(
                                    target: "follow_actor",
                                    error = ?e,
                                    "Failed to send follow status to engine, channel may be closed"
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
