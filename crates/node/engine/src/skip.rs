//! Utility method for checking if payload attributes should be skipped.

use alloy_primitives::keccak256;
use alloy_eips::eip2718::Encodable2718;
use op_alloy_consensus::OpBlock;
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// True if transactions in [OpPayloadAttributes] are not the same as those in a fetched L2
/// [OpBlock]
pub fn should_skip(block: &OpBlock, attributes: &OpPayloadAttributes) -> bool {
    let attributes_hashes = attributes
        .transactions
        .as_ref()
        .unwrap()
        .iter()
        .map(|tx| keccak256(&tx.0))
        .collect::<Vec<_>>();

    let block_hashes =
        block.body.transactions.iter().map(|tx| tx.clone().seal().hash()).collect::<Vec<_>>();

    attributes_hashes == block_hashes
        && attributes.payload_attributes.timestamp == block.header.timestamp
        && attributes.payload_attributes.prev_randao == block.header.mix_hash
        && attributes.payload_attributes.suggested_fee_recipient == block.header.beneficiary
        && attributes.gas_limit.map_or(true, |g| block.header.gas_limit == g)
}
