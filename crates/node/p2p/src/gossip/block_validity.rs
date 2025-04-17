use std::time::SystemTime;

use alloy_consensus::{EMPTY_OMMER_ROOT_HASH, Header};
use alloy_primitives::{Address, B256};
use alloy_rlp::BufMut;
use alloy_rpc_types_engine::{
    ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, PayloadError,
};
use libp2p::gossipsub::MessageAcceptance;
use op_alloy_rpc_types_engine::{
    OpExecutionPayload, OpExecutionPayloadV4, OpNetworkPayloadEnvelope,
};

use super::BlockHandler;

/// These errors are returned by the [`BlockHandler::block_valid`] method.
/// They specify the reason for which a block has been rejected/ignored.
#[derive(Debug, thiserror::Error)]
pub enum BlockInvalidError {
    #[error("Invalid timestamp. Current: {current}, Received: {received}")]
    Timestamp { current: u64, received: u64 },
    #[error("Base fee per gas overflow")]
    BaseFeePerGasOverflow(#[from] PayloadError),
    #[error("Invalid block hash. Expected: {expected}, Received: {received}")]
    BlockHash { expected: B256, received: B256 },
    #[error("Invalid signature.")]
    Signature,
    #[error("Invalid signer, expected: {expected}, received: {received}")]
    Signer { expected: Address, received: Address },
    // TODO: add the rest of the errors variants in follow-up PRs
}

impl From<BlockInvalidError> for MessageAcceptance {
    fn from(_value: BlockInvalidError) -> Self {
        MessageAcceptance::Reject
    }
}

impl BlockHandler {
    /// Determines if a block is valid.
    ///
    /// We validate the block according to the rules defined here:
    /// <https://specs.optimism.io/protocol/rollup-node-p2p.html#block-validation>
    ///
    /// The block encoding/compression are assumed to be valid at this point (they are first checked
    /// in the handle).
    pub fn block_valid(
        &mut self,
        envelope: &OpNetworkPayloadEnvelope,
    ) -> Result<(), BlockInvalidError> {
        let current_timestamp =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

        // The timestamp is at most 5 seconds in the future.
        let is_future = envelope.payload.timestamp() > current_timestamp + 5;
        // The timestamp is at most 60 seconds in the past.
        let is_past = envelope.payload.timestamp() < current_timestamp - 60;

        // CHECK: The timestamp is not too far in the future or past.
        if is_future || is_past {
            return Err(BlockInvalidError::Timestamp {
                current: current_timestamp,
                received: envelope.payload.timestamp(),
            });
        }

        // CHECK: Ensure the block hash is valid.
        check_block_hash(&envelope.payload, envelope.parent_beacon_block_root)?;

        // CHECK: The signature is valid.
        let msg = envelope.payload_hash.signature_message(self.chain_id);
        let block_signer = *self.signer_recv.borrow();

        // The block has a valid signature.
        let Ok(msg_signer) = envelope.signature.recover_address_from_prehash(&msg) else {
            return Err(BlockInvalidError::Signature);
        };

        // The block is signed by the expected signer (the unsafe block signer).
        if msg_signer != block_signer {
            return Err(BlockInvalidError::Signer { expected: msg_signer, received: block_signer });
        }

        Ok(())
    }
}

/// Checks that the block hash is valid. For that we need to recreate the header, recompute the
/// associated block hash and compare it with the one in the block.
pub fn check_block_hash(
    block: &OpExecutionPayload,
    parent_beacon_block_root: Option<B256>,
) -> Result<(), BlockInvalidError> {
    // Computes the associated block header given an [`ExecutionPayloadV1`].
    let block_v1_header = |block: &ExecutionPayloadV1| -> Result<Header, PayloadError> {
        // Reuse the encoded bytes for root calculation
        let transactions_root = alloy_consensus::proofs::ordered_trie_root_with_encoder(
            &block.transactions,
            |item, buf| buf.put_slice(item),
        );

        let header = Header {
            parent_hash: block.parent_hash,
            beneficiary: block.fee_recipient,
            state_root: block.state_root,
            transactions_root,
            receipts_root: block.receipts_root,
            withdrawals_root: None,
            logs_bloom: block.logs_bloom,
            number: block.block_number,
            gas_limit: block.gas_limit,
            gas_used: block.gas_used,
            timestamp: block.timestamp,
            mix_hash: block.prev_randao,
            // WARNING: It’s allowed for a base fee in EIP1559 to increase unbounded. We assume that
            // it will fit in an u64. This is not always necessarily true, although it is extremely
            // unlikely not to be the case, a u64 maximum would have 2^64 which equates to 18 ETH
            // per gas.
            base_fee_per_gas: Some(
                block
                    .base_fee_per_gas
                    .try_into()
                    .map_err(|_| PayloadError::BaseFee(block.base_fee_per_gas))?,
            ),
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root,
            requests_hash: None,
            // Unfortunately we need to clone the extra data because we only have a reference to it.
            extra_data: block.extra_data.clone(),
            // Defaults
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            difficulty: Default::default(),
            nonce: Default::default(),
        };

        Ok(header)
    };

    // Computes the associated block header given an [`ExecutionPayloadV2`].
    // Basically the same as [`block_v1_header`], but with the withdrawals root.
    let block_v2_header = |block: &ExecutionPayloadV2,
                           withdrawals_hint: Option<B256>|
     -> Result<Header, PayloadError> {
        let mut header = block_v1_header(&block.payload_inner)?;

        let withdrawals_root = withdrawals_hint.unwrap_or_else(|| {
            alloy_consensus::proofs::calculate_withdrawals_root(&block.withdrawals)
        });
        header.withdrawals_root = Some(withdrawals_root);

        Ok(header)
    };

    // Computes the associated block header given an [`ExecutionPayloadV3`].
    // Basically the same as [`block_v2_header`], but with the excess blob gas and blob gas used.
    let block_v3_header = |block: &ExecutionPayloadV3,
                           withdrawals_hint: Option<B256>|
     -> Result<Header, PayloadError> {
        let mut header = block_v2_header(&block.payload_inner, withdrawals_hint)?;

        header.blob_gas_used = Some(block.blob_gas_used);
        header.excess_blob_gas = Some(block.excess_blob_gas);

        Ok(header)
    };

    let block_v4_header = |block: &OpExecutionPayloadV4| -> Result<Header, PayloadError> {
        let header = block_v3_header(&block.payload_inner, Some(block.withdrawals_root))?;

        Ok(header)
    };

    let header = match block {
        OpExecutionPayload::V1(payload) => block_v1_header(payload),
        OpExecutionPayload::V2(payload) => block_v2_header(payload, None),
        OpExecutionPayload::V3(payload) => block_v3_header(payload, None),
        OpExecutionPayload::V4(payload) => block_v4_header(payload),
    }?;

    let block_hash = block.block_hash();
    let real_header_hash = header.hash_slow();

    if real_header_hash != block_hash {
        return Err(BlockInvalidError::BlockHash {
            expected: block_hash,
            received: real_header_hash,
        });
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use alloy_consensus::{Block, EMPTY_OMMER_ROOT_HASH};
    use alloy_eips::eip2718::Encodable2718;
    use alloy_primitives::{B256, Bytes, Signature};
    use alloy_rlp::BufMut;
    use alloy_rpc_types_engine::{ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3};
    use arbitrary::{Arbitrary, Unstructured};
    use op_alloy_consensus::OpTxEnvelope;
    use op_alloy_rpc_types_engine::{OpExecutionPayload, OpExecutionPayloadV4, PayloadHash};

    fn valid_block() -> Block<OpTxEnvelope> {
        // Simulate some random data
        let mut data = vec![0; 1024 * 1024];
        let mut rng = rand::rng();
        rand::Rng::fill(&mut rng, &mut data[..]);

        // Create unstructured data with the random bytes
        let u = Unstructured::new(&data);

        // Generate a random instance of MyStruct
        let mut block: Block<OpTxEnvelope> = Block::arbitrary_take_rest(u).unwrap();

        let transactions: Vec<Bytes> =
            block.body.transactions().map(|tx| tx.encoded_2718().into()).collect();

        let transactions_root =
            alloy_consensus::proofs::ordered_trie_root_with_encoder(&transactions, |item, buf| {
                buf.put_slice(item)
            });

        block.header.transactions_root = transactions_root;

        // That is an edge case: if the base fee per gas is not set, the block will be rejected
        // because we don't know if the field is `None` or `Some(0)`. By default, we assume
        // it is `Some(0)` See the conversion here: `<https://github.com/alloy-rs/alloy/blob/033878d34177450028d1d37afd0fd20e08c99244/crates/rpc-types-engine/src/payload.rs#L345>`
        block.header.base_fee_per_gas = Some(block.header.base_fee_per_gas.unwrap_or_default());

        let current_timestamp =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        block.header.timestamp = current_timestamp;

        block
    }

    /// Make the block v1 compatible
    fn v1_valid_block() -> Block<OpTxEnvelope> {
        let mut block = valid_block();
        block.header.withdrawals_root = None;
        block.header.blob_gas_used = None;
        block.header.excess_blob_gas = None;
        block.header.parent_beacon_block_root = None;
        block.header.requests_hash = None;
        block.header.ommers_hash = EMPTY_OMMER_ROOT_HASH;
        block.header.difficulty = Default::default();
        block.header.nonce = Default::default();

        block
    }

    /// Make the block v2 compatible
    pub fn v2_valid_block() -> Block<OpTxEnvelope> {
        let mut block = v1_valid_block();

        block.body.withdrawals = Some(vec![].into());
        let withdrawals_root = alloy_consensus::proofs::calculate_withdrawals_root(
            &block.body.withdrawals.clone().unwrap_or_default(),
        );

        block.header.withdrawals_root = Some(withdrawals_root);

        block
    }

    /// Make the block v3 compatible
    pub fn v3_valid_block() -> Block<OpTxEnvelope> {
        let mut block = valid_block();

        block.body.withdrawals = Some(vec![].into());
        let withdrawals_root = alloy_consensus::proofs::calculate_withdrawals_root(
            &block.body.withdrawals.clone().unwrap_or_default(),
        );
        block.header.withdrawals_root = Some(withdrawals_root);

        block.header.blob_gas_used = Some(0);
        block.header.excess_blob_gas = Some(0);
        block.header.parent_beacon_block_root =
            Some(block.header.parent_beacon_block_root.unwrap_or_default());

        block.header.requests_hash = None;
        block.header.ommers_hash = EMPTY_OMMER_ROOT_HASH;
        block.header.difficulty = Default::default();
        block.header.nonce = Default::default();

        block
    }

    /// Make the block v4 compatible
    pub fn v4_valid_block() -> Block<OpTxEnvelope> {
        v3_valid_block()
    }

    /// Generates a random valid block and ensure it is v1 compatible
    #[test]
    fn test_block_valid() {
        let block = v1_valid_block();

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(handler.block_valid(&envelope).is_ok());
    }

    /// Generates a random block with an invalid timestamp and ensure it is rejected
    #[test]
    fn test_block_invalid_timestamp_early() {
        let mut block = v1_valid_block();

        block.header.timestamp -= 61;

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Timestamp { .. })));
    }

    /// Generates a random block with an invalid timestamp and ensure it is rejected
    #[test]
    fn test_block_invalid_timestamp_too_far() {
        let mut block = v1_valid_block();

        block.header.timestamp += 6;

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Timestamp { .. })));
    }

    /// Generates a random block with an invalid hash and ensure it is rejected
    #[test]
    fn test_block_invalid_hash() {
        let block = v1_valid_block();

        let mut v1 = ExecutionPayloadV1::from_block_slow(&block);

        v1.block_hash = B256::ZERO;

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::BlockHash { .. })));
    }

    #[test]
    fn test_v2_block() {
        let block = v2_valid_block();

        let v2 = ExecutionPayloadV2::from_block_slow(&block);

        let payload = OpExecutionPayload::V2(v2);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(handler.block_valid(&envelope).is_ok());
    }

    #[test]
    fn test_v3_block() {
        let block = v3_valid_block();

        let v3 = ExecutionPayloadV3::from_block_slow(&block);

        let payload = OpExecutionPayload::V3(v3);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: Some(
                block.header.parent_beacon_block_root.unwrap_or_default(),
            ),
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(handler.block_valid(&envelope).is_ok());
    }

    #[test]
    fn test_v4_block() {
        let block = v4_valid_block();

        let v3 = ExecutionPayloadV3::from_block_slow(&block);
        let v4 = OpExecutionPayloadV4::from_v3_with_withdrawals_root(
            v3,
            block.withdrawals_root.unwrap(),
        );

        let payload = OpExecutionPayload::V4(v4);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: Some(
                block.header.parent_beacon_block_root.unwrap_or_default(),
            ),
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(handler.block_valid(&envelope).is_ok());
    }
}
