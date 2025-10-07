//! Compatibility layer for converting between op-alloy 0.20.0 and 0.21.0 types
//!
//! This module provides conversions between the two versions of op-alloy types
//! that are currently in use due to reth v1.8.2 depending on 0.20.0 while
//! we want to use 0.21.0 in our public API.
//!
//! The strategy is to use reth's re-exported types (v0.20.0) when interacting
//! with reth APIs, and provide conversion functions when needed.

use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelopeV3 as OpExecutionPayloadEnvelopeV3_21;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelopeV4 as OpExecutionPayloadEnvelopeV4_21;
use op_alloy_rpc_types_engine::OpExecutionPayloadV4 as OpExecutionPayloadV4_21;
use op_alloy_rpc_types_engine::OpPayloadAttributes as OpPayloadAttributes21;
use op_alloy_rpc_types_engine_v20::OpExecutionPayloadEnvelopeV3 as OpExecutionPayloadEnvelopeV3_20;
use op_alloy_rpc_types_engine_v20::OpExecutionPayloadEnvelopeV4 as OpExecutionPayloadEnvelopeV4_20;
use op_alloy_rpc_types_engine_v20::OpExecutionPayloadV4 as OpExecutionPayloadV4_20;
use op_alloy_rpc_types_engine_v20::OpPayloadAttributes as OpPayloadAttributes20;

/// Convert from op-alloy 0.21.0 OpPayloadAttributes to 0.20.0 version
/// used by reth v1.8.2
///
/// This conversion is safe because the types are structurally identical
/// between versions - only the crate version differs.
pub fn to_v20_payload_attributes(attr: &OpPayloadAttributes21) -> OpPayloadAttributes20 {
    OpPayloadAttributes20 {
        payload_attributes: attr.payload_attributes.clone(),
        gas_limit: attr.gas_limit,
        eip_1559_params: attr.eip_1559_params,
        transactions: attr.transactions.clone(),
        no_tx_pool: attr.no_tx_pool,
        min_base_fee: attr.min_base_fee,
    }
}

/// Convert from op-alloy 0.21.0 OpExecutionPayloadV4 to 0.20.0 version
/// used by rollup-boost/reth
///
/// This conversion is safe because the types are structurally identical
/// between versions - only the crate version differs.
pub fn to_v20_execution_payload_v4(payload: OpExecutionPayloadV4_21) -> OpExecutionPayloadV4_20 {
    OpExecutionPayloadV4_20 {
        payload_inner: payload.payload_inner,
        withdrawals_root: payload.withdrawals_root,
    }
}

/// Convert from op-alloy 0.20.0 OpExecutionPayloadV4 to 0.21.0 version
/// used by rollup-boost/reth
///
/// This conversion is safe because the types are structurally identical
/// between versions - only the crate version differs.
pub fn to_v21_execution_payload_v4(payload: OpExecutionPayloadV4_20) -> OpExecutionPayloadV4_21 {
    OpExecutionPayloadV4_21 {
        payload_inner: payload.payload_inner,
        withdrawals_root: payload.withdrawals_root,
    }
}

/// Convert from op-alloy 0.20.0 OpExecutionPayloadEnvelopeV3 to 0.21.0 version
///
/// This conversion is safe because the types are structurally identical
/// between versions - only the crate version differs.
pub fn to_v21_execution_payload_envelope_v3(
    envelope: OpExecutionPayloadEnvelopeV3_20,
) -> OpExecutionPayloadEnvelopeV3_21 {
    OpExecutionPayloadEnvelopeV3_21 {
        execution_payload: envelope.execution_payload,
        block_value: envelope.block_value,
        blobs_bundle: envelope.blobs_bundle,
        should_override_builder: envelope.should_override_builder,
        parent_beacon_block_root: envelope.parent_beacon_block_root,
    }
}

/// Convert from op-alloy 0.20.0 OpExecutionPayloadEnvelopeV4 to 0.21.0 version
///
/// This conversion is safe because the types are structurally identical
/// between versions - only the crate version differs.
pub fn to_v21_execution_payload_envelope_v4(
    envelope: OpExecutionPayloadEnvelopeV4_20,
) -> OpExecutionPayloadEnvelopeV4_21 {
    OpExecutionPayloadEnvelopeV4_21 {
        execution_payload: to_v21_execution_payload_v4(envelope.execution_payload),
        block_value: envelope.block_value,
        blobs_bundle: envelope.blobs_bundle,
        should_override_builder: envelope.should_override_builder,
        parent_beacon_block_root: envelope.parent_beacon_block_root,
        execution_requests: envelope.execution_requests,
    }
}
