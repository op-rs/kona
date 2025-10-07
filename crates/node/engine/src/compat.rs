//! Compatibility layer between `op-alloy-rpc-types-engine` v0.20.0 and
//! v_later, where v_later is an alias for the current direct dependency version
//! of op-alloy-rpc-types-engine in Cargo.toml.
//!
//! - Reth v1.8.3 depends on v0.20.0; our public API uses v_later.
//! - Use v0.20.0 types at reth boundaries and convert to/from v0.22.0 here.
//! - If upstream types diverge, compile-time assertions in the tests will fail, causing a build
//!   error (preferred over runtime breakage).

use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3 as OpExecutionPayloadEnvelopeV3_later,
    OpExecutionPayloadEnvelopeV4 as OpExecutionPayloadEnvelopeV4_later,
    OpExecutionPayloadV4 as OpExecutionPayloadV4_later,
    OpPayloadAttributes as OpPayloadAttributes_later,
};
use op_alloy_rpc_types_engine_v20::{
    OpExecutionPayloadEnvelopeV3 as OpExecutionPayloadEnvelopeV3_20,
    OpExecutionPayloadEnvelopeV4 as OpExecutionPayloadEnvelopeV4_20,
    OpExecutionPayloadV4 as OpExecutionPayloadV4_20, OpPayloadAttributes as OpPayloadAttributes20,
};

/// Convert v_later `OpPayloadAttributes` to v0.20.0 (for reth).
pub fn to_v20_payload_attributes(attr: &OpPayloadAttributes_later) -> OpPayloadAttributes20 {
    OpPayloadAttributes20 {
        payload_attributes: attr.payload_attributes.clone(),
        gas_limit: attr.gas_limit,
        eip_1559_params: attr.eip_1559_params,
        transactions: attr.transactions.clone(),
        no_tx_pool: attr.no_tx_pool,
        min_base_fee: attr.min_base_fee,
    }
}

/// Convert v_later `OpExecutionPayloadV4` to v0.20.0.
pub fn to_v20_execution_payload_v4(payload: OpExecutionPayloadV4_later) -> OpExecutionPayloadV4_20 {
    OpExecutionPayloadV4_20 {
        payload_inner: payload.payload_inner,
        withdrawals_root: payload.withdrawals_root,
    }
}

/// Convert v0.20.0 `OpExecutionPayloadV4` to v_later.
pub fn to_v_later_execution_payload_v4(
    payload: OpExecutionPayloadV4_20,
) -> OpExecutionPayloadV4_later {
    OpExecutionPayloadV4_later {
        payload_inner: payload.payload_inner,
        withdrawals_root: payload.withdrawals_root,
    }
}

/// Convert v0.20.0 `OpExecutionPayloadEnvelopeV3` to v_later.
pub fn to_v_later_execution_payload_envelope_v3(
    envelope: OpExecutionPayloadEnvelopeV3_20,
) -> OpExecutionPayloadEnvelopeV3_later {
    OpExecutionPayloadEnvelopeV3_later {
        execution_payload: envelope.execution_payload,
        block_value: envelope.block_value,
        blobs_bundle: envelope.blobs_bundle,
        should_override_builder: envelope.should_override_builder,
        parent_beacon_block_root: envelope.parent_beacon_block_root,
    }
}

/// Convert v0.20.0 `OpExecutionPayloadEnvelopeV4` to v_later.
pub fn to_v_later_execution_payload_envelope_v4(
    envelope: OpExecutionPayloadEnvelopeV4_20,
) -> OpExecutionPayloadEnvelopeV4_later {
    OpExecutionPayloadEnvelopeV4_later {
        execution_payload: to_v_later_execution_payload_v4(envelope.execution_payload),
        block_value: envelope.block_value,
        blobs_bundle: envelope.blobs_bundle,
        should_override_builder: envelope.should_override_builder,
        parent_beacon_block_root: envelope.parent_beacon_block_root,
        execution_requests: envelope.execution_requests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are compile-time assertions that the conversion function signatures remain valid.
    // If either side's types change in an incompatible way, assignment will fail to type-check.

    // OpPayloadAttributes v_later -> 0.20
    const _: fn(&OpPayloadAttributes_later) -> OpPayloadAttributes20 = to_v20_payload_attributes;

    // OpExecutionPayloadV4 v_later -> 0.20
    const _: fn(OpExecutionPayloadV4_later) -> OpExecutionPayloadV4_20 =
        to_v20_execution_payload_v4;

    // OpExecutionPayloadV4 0.20 -> v_later
    const _: fn(OpExecutionPayloadV4_20) -> OpExecutionPayloadV4_later =
        to_v_later_execution_payload_v4;

    // OpExecutionPayloadEnvelopeV3 0.20 -> v_later
    const _: fn(OpExecutionPayloadEnvelopeV3_20) -> OpExecutionPayloadEnvelopeV3_later =
        to_v_later_execution_payload_envelope_v3;

    // OpExecutionPayloadEnvelopeV4 0.20 -> v_later
    const _: fn(OpExecutionPayloadEnvelopeV4_20) -> OpExecutionPayloadEnvelopeV4_later =
        to_v_later_execution_payload_envelope_v4;
}
