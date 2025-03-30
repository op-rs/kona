//! Contains the accelerated version of the `ecPairing` precompile.

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{
    PrecompileError, PrecompileOutput, PrecompileResult,
    bn128::pair::{self, ISTANBUL_PAIR_BASE, ISTANBUL_PAIR_PER_POINT},
};

const PAIR_ELEMENT_LEN: usize = 64 + 128;

/// Runs the FPVM-accelerated `ecpairing` precompile call.
pub(crate) fn fpvm_bn128_pair<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    let gas_used =
        (input.len() / PAIR_ELEMENT_LEN) as u64 * ISTANBUL_PAIR_PER_POINT + ISTANBUL_PAIR_BASE;

    if gas_used > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    if input.len() % PAIR_ELEMENT_LEN != 0 {
        return Err(PrecompileError::Bn128PairLength);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[pair::ISTANBUL.address().as_slice(), &gas_used.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(gas_used, result_data.into()))
}

/// Runs the FPVM-accelerated `ecpairing` precompile call, with the input size limited by the
/// Granite hardfork.
pub(crate) fn fpvm_bn128_pair_granite<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    const BN256_MAX_PAIRING_SIZE_GRANITE: usize = 112_687;
    if input.len() > BN256_MAX_PAIRING_SIZE_GRANITE {
        return Err(PrecompileError::Bn128PairLength);
    }

    fpvm_bn128_pair(input, gas_limit, hint_writer, oracle_reader)
}
