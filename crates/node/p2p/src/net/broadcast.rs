//! Broadcast handles broadcasting unsafe blocks in-order.

use backon::{ExponentialBuilder, Retryable};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
    time::Duration,
};

/// The `Broadcast` struct is responsible for broadcasting unsafe blocks in order.
#[derive(Debug)]
pub struct Broadcast {
    /// An in-memory buffer to store blocks.
    buffer: VecDeque<OpExecutionPayloadEnvelope>,
    /// The channel to broadcast blocks.
    channel: BroadcastSender<OpExecutionPayloadEnvelope>,
    /// Tracks if broadcasting is being attempted.
    broadcasting: Arc<Mutex<bool>>,
}

impl Broadcast {
    /// Creates a new `Broadcast` instance with the given channel.
    pub fn new(channel: BroadcastSender<OpExecutionPayloadEnvelope>) -> Self {
        Self { buffer: VecDeque::new(), channel, broadcasting: Arc::new(Mutex::new(false)) }
    }

    /// Pushes a new unsafe payload to the buffer.
    pub fn push(&mut self, payload: impl Into<OpExecutionPayloadEnvelope>) {
        self.buffer.push_back(payload.into());
    }

    /// Subscribe to the broadcast channel.
    pub fn subscribe(&self) -> BroadcastReceiver<OpExecutionPayloadEnvelope> {
        self.channel.subscribe()
    }

    /// Broadcasts all unsafe blocks **in order** through the channel.
    ///
    /// If an error occurs, the function will exit early, maintaining the invariant that unsafe
    /// blocks are held in order.
    pub fn broadcast(&mut self) {
        if self.broadcasting.lock().map_or(true, |v| *v) {
            trace!(target: "net", "Broadcasting in flight, skipping");
            return;
        }
        if let Ok(mut broadcasting) = self.broadcasting.lock() {
            *broadcasting = true;
        }
        let flag = Arc::clone(&self.broadcasting);
        let channel = self.channel.clone();
        let mut drained = std::mem::take(&mut self.buffer);
        tokio::spawn(async move {
            loop {
                if drained.is_empty() {
                    break;
                }
                let front = drained.front();
                let fut = || async {
                    if let Some(block) = front {
                        channel.send(block.clone())?;
                    }
                    Ok(())
                };

                let res = fut.retry(ExponentialBuilder::default())
                .notify(|err: &tokio::sync::broadcast::error::SendError<OpExecutionPayloadEnvelope>, dur: Duration| {
                    warn!(target: "net", ?err, "Failed to broadcast block [Duration: {:?}]", dur);
                })
                .await;
                if let Err(e) = res {
                    warn!(target: "net", ?e, "Block broadcasting failed with exponential backoff");
                    break;
                }
                drained.pop_front();
            }
            if let Ok(mut broadcasting) = flag.lock() {
                *broadcasting = false;
            }
        });
    }
}
