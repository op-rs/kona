use std::{collections::HashSet, time::SystemTime};

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
    #[error("Payload is on v2+ topic, but has non-empty withdrawals")]
    Withdrawals,
    #[error("Payload is on v3+ topic, but has empty parent beacon root")]
    ParentBeaconRoot,
    #[error("Payload is on v3+ topic, but has non-zero blob gas used")]
    BlobGasUsed,
    #[error("Payload is on v3+ topic, but has non-zero excess blob gas")]
    ExcessBlobGas,
    #[error("Payload is on v4+ topic, but has non-empty withdrawals root")]
    WithdrawalsRoot,
    #[error("Too many blocks seen for height {height}")]
    TooManyBlocks { height: u64 },
    #[error("Block seen before")]
    BlockSeen { block_hash: B256 },
}

impl From<BlockInvalidError> for MessageAcceptance {
    fn from(value: BlockInvalidError) -> Self {
        // We only want to ignore blocks that we have already seen.
        match value {
            BlockInvalidError::BlockSeen { block_hash: _ } => MessageAcceptance::Ignore,
            _ => MessageAcceptance::Reject,
        }
    }
}

impl BlockHandler {
    /// The maximum number of blocks to keep in the seen hashes map.
    ///
    /// Note: this value must be high enough to ensure we prevent replay attacks.
    /// Ie, the entries pruned must be old enough blocks to be considered invalid
    /// if new blocks for that height are received.
    ///
    /// This value is chosen to match `op-node` validator's lru cache size.
    /// See: `<https://github.com/ethereum-optimism/optimism/blob/836d50be5d5f4ae14ffb2ea6106720a2b080cdae/op-node/p2p/gossip.go#L266>``
    const SEEN_HASH_CACHE_SIZE: usize = 1_000;

    /// The maximum number of blocks to keep per height.
    /// This value is chosen according to the optimism specs:
    /// `<https://specs.optimism.io/protocol/rollup-node-p2p.html#block-validation>`
    const MAX_BLOCKS_TO_KEEP: usize = 5;

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

        // CHECK: The payload is valid for the specific version of this block.
        validate_version_specific_payload(envelope)?;

        let payload_v1 = envelope.payload.as_v1();

        if let Some(seen_hashes_at_height) = self.seen_hashes.get_mut(&payload_v1.block_number) {
            // CHECK: If more than [`Self::MAX_BLOCKS_TO_KEEP`] different blocks have been received
            // for the same height, reject the block.
            if seen_hashes_at_height.len() > Self::MAX_BLOCKS_TO_KEEP {
                return Err(BlockInvalidError::TooManyBlocks { height: payload_v1.block_number });
            }

            // CHECK: If the block has already been seen, ignore it.
            if seen_hashes_at_height.contains(&payload_v1.block_hash) {
                return Err(BlockInvalidError::BlockSeen { block_hash: payload_v1.block_hash });
            }

            seen_hashes_at_height.insert(payload_v1.block_hash);
        } else {
            self.seen_hashes
                .insert(payload_v1.block_number, HashSet::from([payload_v1.block_hash]));
        }

        // CHECK: The signature is valid.
        let msg = envelope.payload_hash.signature_message(self.chain_id);
        let block_signer = *self.signer_recv.borrow();

        // The block has a valid signature.
        let Ok(msg_signer) = envelope.signature.recover_address_from_prehash(&msg) else {
            return Err(BlockInvalidError::Signature);
        };

        // The block is signed by the expected signer (the unsafe block signer).
        if msg_signer != block_signer {
            println!("signer err. {:?} {:?}", msg_signer, block_signer);
            return Err(BlockInvalidError::Signer { expected: msg_signer, received: block_signer });
        }

        // Mark the block as seen.
        if self.seen_hashes.len() >= Self::SEEN_HASH_CACHE_SIZE {
            self.seen_hashes.pop_first();
        }

        Ok(())
    }
}

// Validate version specific contents of the payload.
pub fn validate_version_specific_payload(
    envelope: &OpNetworkPayloadEnvelope,
) -> Result<(), BlockInvalidError> {
    // Validation for v1 payloads are mostly ensured by type-safety, by decoding the
    // payload to the ExecutionPayloadV1 type:
    // 1. The block should not have any withdrawals
    // 2. The block should not have any withdrawals list
    // 3. The block should not have any blob gas used
    // 4. The block should not have any excess blob gas
    // 5. The block should not have any withdrawals root
    // 6. The block should not have any parent beacon block root (validated because ignored by the
    //    decoder, this causes a hash mismatch. See tests)

    // Same as v1, except:
    // 1. The block should have an empty withdrawals list
    fn validate_v2(block: &ExecutionPayloadV2) -> Result<(), BlockInvalidError> {
        if !block.withdrawals.is_empty() {
            return Err(BlockInvalidError::Withdrawals);
        }

        Ok(())
    }

    // Same as v2, except:
    // 1. The block should have a zero blob gas used
    // 2. The block should have a zero excess blob gas
    // 3. The block should have a non empty parent beacon block root
    fn validate_v3(
        block: &ExecutionPayloadV3,
        parent_beacon_block_root: Option<B256>,
    ) -> Result<(), BlockInvalidError> {
        validate_v2(&block.payload_inner)?;

        if block.blob_gas_used != 0 {
            return Err(BlockInvalidError::BlobGasUsed);
        }

        if block.excess_blob_gas != 0 {
            return Err(BlockInvalidError::ExcessBlobGas);
        }

        if parent_beacon_block_root.is_none() {
            return Err(BlockInvalidError::ParentBeaconRoot);
        }

        Ok(())
    }

    // Same as v3, except:
    // 1. The block should have an non-empty withdrawals root (checked by type-safety)
    fn validate_v4(
        block: &OpExecutionPayloadV4,
        parent_beacon_block_root: Option<B256>,
    ) -> Result<(), BlockInvalidError> {
        validate_v3(&block.payload_inner, parent_beacon_block_root)
    }

    match &envelope.payload {
        OpExecutionPayload::V1(_) => Ok(()),
        OpExecutionPayload::V2(payload) => validate_v2(payload),
        OpExecutionPayload::V3(payload) => validate_v3(payload, envelope.parent_beacon_block_root),
        OpExecutionPayload::V4(payload) => validate_v4(payload, envelope.parent_beacon_block_root),
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
    use alloy_eips::{eip2718::Encodable2718, eip4895::Withdrawal};
    use alloy_primitives::{Address, B256, Bytes, Signature};
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
    fn test_cannot_validate_same_block_twice() {
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
        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::BlockSeen { .. })));
    }

    #[test]
    fn test_cannot_have_too_many_blocks_for_the_same_height() {
        let first_block = v1_valid_block();

        let initial_height = first_block.header.number;

        let v1 = ExecutionPayloadV1::from_block_slow(&first_block);

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

        let next_payloads = (0..=BlockHandler::MAX_BLOCKS_TO_KEEP)
            .map(|_| {
                let mut block = v1_valid_block();
                // The blocks have the same height
                block.header.number = initial_height;

                let v1 = ExecutionPayloadV1::from_block_slow(&block);

                let payload = OpExecutionPayload::V1(v1);
                OpNetworkPayloadEnvelope {
                    payload,
                    signature: Signature::test_signature(),
                    payload_hash: PayloadHash(B256::ZERO),
                    parent_beacon_block_root: None,
                }
            })
            .collect::<Vec<_>>();

        for envelope in next_payloads[..next_payloads.len() - 1].iter() {
            assert!(handler.block_valid(envelope).is_ok());
        }

        // The last envelope should fail
        assert!(matches!(
            handler.block_valid(next_payloads.last().unwrap()),
            Err(BlockInvalidError::TooManyBlocks { .. })
        ));
    }

    /// Blocks with invalid signatures should be rejected.
    #[test]
    fn test_invalid_signature() {
        let block = v1_valid_block();

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let mut envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        let mut signature_bytes = envelope.signature.as_bytes();
        signature_bytes[0] = !signature_bytes[0];
        envelope.signature = Signature::from_raw_array(&signature_bytes).unwrap();

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Signature { .. })));
    }

    /// Blocks with invalid signers should be rejected.
    #[test]
    fn test_invalid_signer() {
        let block = v1_valid_block();

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: None,
        };

        let (_, unsafe_signer) = tokio::sync::watch::channel(Address::default());
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Signer { .. })));
    }

    /// If we specify a non empty parent beacon block root for blocks with v1/v2 payloads we
    /// get a hash mismatch error because the decoder enforces that these versions of the execution
    /// payload don't contain the parent beacon block root.
    #[test]
    fn test_v1_v2_block_invalid_parent_beacon_block_root() {
        let block = v1_valid_block();

        let v1 = ExecutionPayloadV1::from_block_slow(&block);

        let payload = OpExecutionPayload::V1(v1);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: Some(B256::ZERO),
        };

        let msg = envelope.payload_hash.signature_message(10);
        let signer = envelope.signature.recover_address_from_prehash(&msg).unwrap();
        let (_, unsafe_signer) = tokio::sync::watch::channel(signer);
        let mut handler = BlockHandler::new(10, unsafe_signer);

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::BlockHash { .. })));

        let block = v1_valid_block();

        let v2 = ExecutionPayloadV2::from_block_slow(&block);

        let payload = OpExecutionPayload::V2(v2);
        let envelope = OpNetworkPayloadEnvelope {
            payload,
            signature: Signature::test_signature(),
            payload_hash: PayloadHash(B256::ZERO),
            parent_beacon_block_root: Some(B256::ZERO),
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
    fn test_v2_non_empty_withdrawals() {
        let mut block = v2_valid_block();
        block.body.withdrawals = Some(vec![Withdrawal::default()].into());
        let withdrawals_root = alloy_consensus::proofs::calculate_withdrawals_root(
            &block.body.withdrawals.clone().unwrap_or_default(),
        );
        block.header.withdrawals_root = Some(withdrawals_root);

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

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Withdrawals)));
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
    fn test_v3_non_empty_withdrawals() {
        let mut block = v3_valid_block();
        block.body.withdrawals = Some(vec![Withdrawal::default()].into());
        let withdrawals_root = alloy_consensus::proofs::calculate_withdrawals_root(
            &block.body.withdrawals.clone().unwrap_or_default(),
        );
        block.header.withdrawals_root = Some(withdrawals_root);

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

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::Withdrawals)));
    }

    #[test]
    fn test_v3_gas_params() {
        let mut block = v3_valid_block();
        block.header.blob_gas_used = Some(1);

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

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::BlobGasUsed)));

        block.header.blob_gas_used = Some(0);
        block.header.excess_blob_gas = Some(1);

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

        assert!(matches!(handler.block_valid(&envelope), Err(BlockInvalidError::ExcessBlobGas)));
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
