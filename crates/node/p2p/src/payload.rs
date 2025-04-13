use alloy_primitives::keccak256;
use alloy_rlp::Encodable;
use op_alloy_consensus::OpPooledTransaction;
use op_alloy_rpc_types_engine::OpExecutionPayload;

/// Trait for validating execution payloads
pub trait PayloadValidation {
    /// Returns true if the payload has withdrawals
    fn has_withdrawals(&self) -> bool;

    /// Returns true if the payload has a withdrawals list
    fn has_withdrawals_list(&self) -> bool;

    /// Returns true if the payload has an empty withdrawals list
    fn has_empty_withdrawals_list(&self) -> bool;

    /// Returns true if the payload has blob gas used
    fn has_blob_gas_used(&self) -> bool;

    /// Returns true if the payload has excess blob gas
    fn has_excess_blob_gas(&self) -> bool;

    /// Returns true if the payload has non-zero blob gas used
    fn has_non_zero_blob_gas_used(&self) -> bool;

    /// Returns true if the payload has non-zero excess blob gas
    fn has_non_zero_excess_blob_gas(&self) -> bool;

    /// Returns true if the block hash in the payload is valid
    fn is_block_hash_valid(&self) -> bool;
}

impl PayloadValidation for OpExecutionPayload {
    fn has_withdrawals(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) => false,
            OpExecutionPayload::V2(payload) => !payload.withdrawals.is_empty(),
            OpExecutionPayload::V3(payload) => !payload.payload_inner.withdrawals.is_empty(),
            OpExecutionPayload::V4(payload) => {
                !payload.payload_inner.payload_inner.withdrawals.is_empty()
            }
        }
    }

    fn has_withdrawals_list(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) => false,
            OpExecutionPayload::V2(_) | OpExecutionPayload::V3(_) | OpExecutionPayload::V4(_) => {
                true
            }
        }
    }

    fn has_empty_withdrawals_list(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) => false,
            OpExecutionPayload::V2(payload) => payload.withdrawals.is_empty(),
            OpExecutionPayload::V3(payload) => payload.payload_inner.withdrawals.is_empty(),
            OpExecutionPayload::V4(payload) => {
                payload.payload_inner.payload_inner.withdrawals.is_empty()
            }
        }
    }

    fn has_blob_gas_used(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) | OpExecutionPayload::V2(_) => false,
            OpExecutionPayload::V3(_) | OpExecutionPayload::V4(_) => true,
        }
    }

    fn has_excess_blob_gas(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) | OpExecutionPayload::V2(_) => false,
            OpExecutionPayload::V3(_) | OpExecutionPayload::V4(_) => true,
        }
    }

    fn has_non_zero_blob_gas_used(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) | OpExecutionPayload::V2(_) => false,
            OpExecutionPayload::V3(payload) => payload.blob_gas_used != 0,
            OpExecutionPayload::V4(payload) => payload.payload_inner.blob_gas_used != 0,
        }
    }

    fn has_non_zero_excess_blob_gas(&self) -> bool {
        match self {
            OpExecutionPayload::V1(_) | OpExecutionPayload::V2(_) => false,
            OpExecutionPayload::V3(payload) => payload.excess_blob_gas != 0,
            OpExecutionPayload::V4(payload) => payload.payload_inner.excess_blob_gas != 0,
        }
    }

    fn is_block_hash_valid(&self) -> bool {
        let block_result = self.clone().try_into_block::<OpPooledTransaction>();
        match block_result {
            Ok(block) => {
                let mut buf = Vec::new();
                block.header.encode(&mut buf);
                let computed_hash = keccak256(&buf);
                computed_hash == self.as_v1().block_hash
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256, Bloom, Bytes, U256};
    use alloy_rpc_types_engine::{ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3};
    use op_alloy_rpc_types_engine::OpExecutionPayloadV4;

    #[test]
    fn test_withdrawals() {
        let v1 = OpExecutionPayload::V1(ExecutionPayloadV1 {
            parent_hash: B256::ZERO,
            fee_recipient: Address::default(),
            state_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom: Bloom::default(),
            prev_randao: B256::ZERO,
            block_number: 0,
            gas_limit: 0,
            gas_used: 0,
            timestamp: 0,
            extra_data: Bytes::default(),
            base_fee_per_gas: U256::from(0),
            block_hash: B256::ZERO,
            transactions: vec![],
        });
        assert!(!v1.has_withdrawals());
        assert!(!v1.has_withdrawals_list());
        assert!(!v1.has_empty_withdrawals_list());

        let v2 = OpExecutionPayload::V2(ExecutionPayloadV2 {
            withdrawals: vec![],
            payload_inner: ExecutionPayloadV1 {
                parent_hash: B256::ZERO,
                fee_recipient: Address::default(),
                state_root: B256::ZERO,
                receipts_root: B256::ZERO,
                logs_bloom: Bloom::default(),
                prev_randao: B256::ZERO,
                block_number: 0,
                gas_limit: 0,
                gas_used: 0,
                timestamp: 0,
                extra_data: Bytes::default(),
                base_fee_per_gas: U256::from(0),
                block_hash: B256::ZERO,
                transactions: vec![],
            },
        });
        assert!(!v2.has_withdrawals());
        assert!(v2.has_withdrawals_list());
        assert!(v2.has_empty_withdrawals_list());

        let v3 = OpExecutionPayload::V3(ExecutionPayloadV3 {
            payload_inner: ExecutionPayloadV2 {
                withdrawals: vec![],
                payload_inner: ExecutionPayloadV1 {
                    parent_hash: B256::ZERO,
                    fee_recipient: Address::default(),
                    state_root: B256::ZERO,
                    receipts_root: B256::ZERO,
                    logs_bloom: Bloom::default(),
                    prev_randao: B256::ZERO,
                    block_number: 0,
                    gas_limit: 0,
                    gas_used: 0,
                    timestamp: 0,
                    extra_data: Bytes::default(),
                    base_fee_per_gas: U256::from(0),
                    block_hash: B256::ZERO,
                    transactions: vec![],
                },
            },
            blob_gas_used: 0,
            excess_blob_gas: 0,
        });
        assert!(!v3.has_withdrawals());
        assert!(v3.has_withdrawals_list());
        assert!(v3.has_empty_withdrawals_list());
        assert!(v3.has_blob_gas_used());
        assert!(v3.has_excess_blob_gas());
        assert!(!v3.has_non_zero_blob_gas_used());
        assert!(!v3.has_non_zero_excess_blob_gas());

        let v4 = OpExecutionPayload::V4(OpExecutionPayloadV4 {
            payload_inner: ExecutionPayloadV3 {
                payload_inner: ExecutionPayloadV2 {
                    withdrawals: vec![],
                    payload_inner: ExecutionPayloadV1 {
                        parent_hash: B256::ZERO,
                        fee_recipient: Address::default(),
                        state_root: B256::ZERO,
                        receipts_root: B256::ZERO,
                        logs_bloom: Bloom::default(),
                        prev_randao: B256::ZERO,
                        block_number: 0,
                        gas_limit: 0,
                        gas_used: 0,
                        timestamp: 0,
                        extra_data: Bytes::default(),
                        base_fee_per_gas: U256::from(0),
                        block_hash: B256::ZERO,
                        transactions: vec![],
                    },
                },
                blob_gas_used: 1,
                excess_blob_gas: 1,
            },
            withdrawals_root: B256::ZERO,
        });
        assert!(!v4.has_withdrawals());
        assert!(v4.has_withdrawals_list());
        assert!(v4.has_empty_withdrawals_list());
        assert!(v4.has_blob_gas_used());
        assert!(v4.has_excess_blob_gas());
        assert!(v4.has_non_zero_blob_gas_used());
        assert!(v4.has_non_zero_excess_blob_gas());
    }
}
