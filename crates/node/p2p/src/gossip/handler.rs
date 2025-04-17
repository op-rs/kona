//! Block Handler

use crate::HandlerEncodeError;
use alloy_primitives::Address;
use kona_genesis::RollupConfig;
use libp2p::gossipsub::{IdentTopic, Message, MessageAcceptance, TopicHash};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::sync::watch::Receiver;

/// This trait defines the functionality required to process incoming messages
/// and determine their acceptance within the network.
///
/// Implementors of this trait can specify how messages are handled and which
/// topics they are interested in.
pub trait Handler: Send {
    /// Manages validation and further processing of messages
    /// This is a stateful method, because the handler needs to keep track of seen hashes.
    fn handle(&mut self, msg: Message) -> (MessageAcceptance, Option<OpNetworkPayloadEnvelope>);

    /// Specifies which topics the handler is interested in
    fn topics(&self) -> Vec<TopicHash>;
}

/// Responsible for managing blocks received via p2p gossip
#[derive(Debug, Clone)]
pub struct BlockHandler {
    /// Chain ID.
    ///
    /// Used to filter out gossip messages intended for other chains.
    pub chain_id: u64,
    /// A [`Receiver`] to monitor changes to the unsafe block signer.
    pub signer_recv: Receiver<Address>,
    /// The libp2p topic for pre Canyon/Shangai blocks.
    pub blocks_v1_topic: IdentTopic,
    /// The libp2p topic for Canyon/Delta blocks.
    pub blocks_v2_topic: IdentTopic,
    /// The libp2p topic for Ecotone V3 blocks.
    pub blocks_v3_topic: IdentTopic,
    /// The libp2p topic for V4 blocks.
    pub blocks_v4_topic: IdentTopic,
}

impl Handler for BlockHandler {
    /// Checks validity of a [`OpNetworkPayloadEnvelope`] received over P2P gossip.
    /// If valid, sends the [`OpNetworkPayloadEnvelope`] to the block update channel.
    fn handle(&mut self, msg: Message) -> (MessageAcceptance, Option<OpNetworkPayloadEnvelope>) {
        let decoded = if msg.topic == self.blocks_v1_topic.hash() {
            OpNetworkPayloadEnvelope::decode_v1(&msg.data)
        } else if msg.topic == self.blocks_v2_topic.hash() {
            OpNetworkPayloadEnvelope::decode_v2(&msg.data)
        } else if msg.topic == self.blocks_v3_topic.hash() {
            OpNetworkPayloadEnvelope::decode_v3(&msg.data)
        } else if msg.topic == self.blocks_v4_topic.hash() {
            OpNetworkPayloadEnvelope::decode_v4(&msg.data)
        } else {
            warn!(target: "node::p2p::gossip", topic = ?msg.topic, "Received block with unknown topic");
            return (MessageAcceptance::Reject, None);
        };

        match decoded {
            Ok(envelope) => match self.block_valid(&envelope) {
                Ok(()) => (MessageAcceptance::Accept, Some(envelope)),
                Err(err) => {
                    warn!(target: "node::p2p::gossip", ?err, ?envelope, "Received invalid block");
                    (err.into(), None)
                }
            },
            Err(err) => {
                warn!(target: "node::p2p::gossip", ?err, "Failed to decode block");
                (MessageAcceptance::Reject, None)
            }
        }
    }

    /// The gossip topics accepted for new blocks
    fn topics(&self) -> Vec<TopicHash> {
        vec![
            self.blocks_v1_topic.hash(),
            self.blocks_v2_topic.hash(),
            self.blocks_v3_topic.hash(),
            self.blocks_v4_topic.hash(),
        ]
    }
}

impl BlockHandler {
    /// Creates a new [`BlockHandler`].
    ///
    /// Requires the chain ID and a receiver channel for the unsafe block signer.
    pub fn new(chain_id: u64, signer_recv: Receiver<Address>) -> Self {
        Self {
            chain_id,
            signer_recv,
            blocks_v1_topic: IdentTopic::new(format!("/optimism/{}/0/blocks", chain_id)),
            blocks_v2_topic: IdentTopic::new(format!("/optimism/{}/1/blocks", chain_id)),
            blocks_v3_topic: IdentTopic::new(format!("/optimism/{}/2/blocks", chain_id)),
            blocks_v4_topic: IdentTopic::new(format!("/optimism/{}/3/blocks", chain_id)),
        }
    }

    /// Returns the topic using the specified timestamp and optional [`RollupConfig`].
    ///
    /// Reference: <https://github.com/ethereum-optimism/optimism/blob/0bc5fe8d16155dc68bcdf1fa5733abc58689a618/op-node/p2p/gossip.go#L604C1-L612C3>
    pub fn topic(&self, timestamp: u64, cfg: Option<&RollupConfig>) -> IdentTopic {
        let Some(cfg) = cfg else {
            return self.blocks_v4_topic.clone();
        };

        if cfg.is_isthmus_active(timestamp) {
            self.blocks_v4_topic.clone()
        } else if cfg.is_ecotone_active(timestamp) {
            self.blocks_v3_topic.clone()
        } else if cfg.is_canyon_active(timestamp) {
            self.blocks_v2_topic.clone()
        } else {
            self.blocks_v1_topic.clone()
        }
    }

    /// Encodes a [`OpNetworkPayloadEnvelope`] into a byte array
    /// based on the specified topic.
    pub fn encode(
        &self,
        topic: IdentTopic,
        envelope: OpNetworkPayloadEnvelope,
    ) -> Result<Vec<u8>, HandlerEncodeError> {
        let encoded = match topic.hash() {
            hash if hash == self.blocks_v1_topic.hash() => envelope.encode_v1()?,
            hash if hash == self.blocks_v2_topic.hash() => envelope.encode_v2()?,
            hash if hash == self.blocks_v3_topic.hash() => envelope.encode_v3()?,
            hash if hash == self.blocks_v4_topic.hash() => envelope.encode_v4()?,
            hash => return Err(HandlerEncodeError::UnknownTopic(hash)),
        };
        Ok(encoded)
    }
}
