//! Block Handler

use alloy_primitives::Address;
use libp2p::gossipsub::{IdentTopic, Message, MessageAcceptance, TopicHash};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::time::SystemTime;
use tokio::sync::{
    mpsc::{Sender, channel},
    watch::Receiver,
};

/// This trait defines the functionality required to process incoming messages
/// and determine their acceptance within the network.
///
/// Implementors of this trait can specify how messages are handled and which
/// topics they are interested in.
pub trait Handler: Send {
    /// Manages validation and further processing of messages
    fn handle(&self, msg: Message) -> MessageAcceptance;

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
    /// A channel [`Sender`] to forward new [`OpNetworkPayloadEnvelope`] to other modules.
    pub block_sender: Sender<OpNetworkPayloadEnvelope>,
    /// A [`Receiver`] to monitor changes to the unsafe block signer.
    pub unsafe_signer_recv: Receiver<Address>,
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
    /// Checks validity of a block received via p2p gossip, and sends to the block update channel if
    /// valid.
    fn handle(&self, msg: Message) -> MessageAcceptance {
        let decoded = if msg.topic == self.blocks_v1_topic.hash() {
            info!("received v1 block");
            OpNetworkPayloadEnvelope::decode_v1(&msg.data)
        } else if msg.topic == self.blocks_v2_topic.hash() {
            info!("received v2 block");
            OpNetworkPayloadEnvelope::decode_v2(&msg.data)
        } else if msg.topic == self.blocks_v3_topic.hash() {
            info!("received v3 block");
            OpNetworkPayloadEnvelope::decode_v3(&msg.data)
        } else if msg.topic == self.blocks_v4_topic.hash() {
            info!("received v4 block");
            return MessageAcceptance::Reject;
            // OpNetworkPayloadEnvelope::decode_v4(&msg.data)
        } else {
            warn!("Received block with unknown topic: {:?}", msg.topic);
            return MessageAcceptance::Reject;
        };

        match decoded {
            Ok(envelope) => {
                if self.block_valid(&envelope) {
                    _ = self.block_sender.send(envelope);
                    MessageAcceptance::Accept
                } else {
                    debug!("Invalid block received");
                    MessageAcceptance::Reject
                }
            }
            Err(err) => {
                debug!("Failed to decode block: {:?}", err);
                MessageAcceptance::Reject
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
    /// Creates a new [BlockHandler] and opens a channel
    pub fn new(
        chain_id: u64,
        unsafe_recv: Receiver<Address>,
    ) -> (Self, tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>) {
        let (sender, recv) = channel(256);

        let handler = Self {
            chain_id,
            block_sender: sender,
            unsafe_signer_recv: unsafe_recv,
            blocks_v1_topic: IdentTopic::new(format!("/optimism/{}/0/blocks", chain_id)),
            blocks_v2_topic: IdentTopic::new(format!("/optimism/{}/1/blocks", chain_id)),
            blocks_v3_topic: IdentTopic::new(format!("/optimism/{}/2/blocks", chain_id)),
            blocks_v4_topic: IdentTopic::new(format!("/optimism/{}/3/blocks", chain_id)),
        };

        (handler, recv)
    }

    /// Determines if a block is valid.
    ///
    /// True if the block is less than 1 minute old, and correctly signed by the unsafe block
    /// signer.
    pub fn block_valid(&self, envelope: &OpNetworkPayloadEnvelope) -> bool {
        let current_timestamp =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

        let is_future = envelope.payload.timestamp() > current_timestamp + 5;
        let is_past = envelope.payload.timestamp() < current_timestamp - 60;
        let time_valid = !(is_future || is_past);

        let msg = envelope.payload_hash.signature_message(self.chain_id);
        let block_signer = *self.unsafe_signer_recv.borrow();
        let Ok(msg_signer) = envelope.signature.recover_address_from_prehash(&msg) else {
            warn!(target: "p2p::block_handler", "Failed to recover address from message");
            return false;
        };

        let signer_valid = msg_signer == block_signer;
        time_valid && signer_valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256, Bloom, Bytes, PrimitiveSignature, U256};
    use alloy_rpc_types_engine::ExecutionPayloadV1;
    use op_alloy_rpc_types_engine::{OpExecutionPayload, PayloadHash};

    #[test]
    fn test_block_valid() {
        let current_timestamp =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

        let v1 = ExecutionPayloadV1 {
            parent_hash: B256::ZERO,
            fee_recipient: Address::default(),
            state_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom: Bloom::default(),
            prev_randao: B256::ZERO,
            block_number: 0,
            gas_limit: 0,
            gas_used: 0,
            timestamp: current_timestamp,
            extra_data: Bytes::default(),
            base_fee_per_gas: U256::from(0),
            block_hash: B256::ZERO,
            transactions: vec![],
        };
        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: PrimitiveSignature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let (handler, _) = BlockHandler::new(10, unsafe_signer);

        assert!(handler.block_valid(&envelope));
    }
}
