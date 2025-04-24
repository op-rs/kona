//! Broadcast handles broadcasting unsafe blocks in-order.

use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::collections::VecDeque;
use tokio::sync::broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender};

/// The `Broadcast` struct is responsible for broadcasting unsafe blocks in order.
#[derive(Debug)]
pub struct Broadcast {
    /// An in-memory buffer to store blocks.
    buffer: VecDeque<OpNetworkPayloadEnvelope>,
    /// The channel to broadcast blocks.
    channel: BroadcastSender<OpNetworkPayloadEnvelope>,
}

impl Broadcast {
    /// Creates a new `Broadcast` instance with the given channel.
    pub const fn new(channel: BroadcastSender<OpNetworkPayloadEnvelope>) -> Self {
        Self { buffer: VecDeque::new(), channel }
    }

    /// Pushes a new unsafe block to the buffer.
    pub fn push(&mut self, block: OpNetworkPayloadEnvelope) {
        self.buffer.push_back(block);
    }

    /// Subscribe to the broadcast channel.
    pub fn subscribe(&self) -> BroadcastReceiver<OpNetworkPayloadEnvelope> {
        self.channel.subscribe()
    }

    /// Broadcasts all unsafe blocks **in order** through the channel.
    ///
    /// If an error occurs, the function will exit early, maintaining the invariant that unsafe
    /// blocks are held in order.
    pub fn broadcast(&mut self) {
        loop {
            if self.buffer.is_empty() {
                break;
            }
            if let Some(block) = self.buffer.front() {
                if let Err(e) = self.channel.send(block.clone()) {
                    trace!("Failed to send unsafe block through channel: {:?}", e);
                    break;
                }
                self.buffer.pop_front();
            }
        }
    }
}
