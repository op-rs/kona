//! Contains the accelerated precompile for the BLS12-381 curve.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/pairing.rs

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{
    PrecompileError, PrecompileOutput, PrecompileResult, bls12_381,
    bls12_381_const::{PAIRING_INPUT_LENGTH, PAIRING_MULTIPLIER_BASE, PAIRING_OFFSET_BASE},
};

/// The max pairing size for BLS12-381 input given a 20M gas limit.
const BLS12_MAX_PAIRING_SIZE_ISTHMUS: usize = 235_008;

/// Performs an FPVM-accelerated BLS12-381 pairing check.
pub(crate) fn fpvm_bls12_pairing<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    let input_len = input.len();

    if input_len > BLS12_MAX_PAIRING_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "Pairing input length must be at most {}",
            BLS12_MAX_PAIRING_SIZE_ISTHMUS
        )));
    }

    if input_len % PAIRING_INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "Pairing input length should be multiple of {PAIRING_INPUT_LENGTH}, was {input_len}"
        )));
    }

    let k = input_len / PAIRING_INPUT_LENGTH;
    let required_gas: u64 = PAIRING_MULTIPLIER_BASE * k as u64 + PAIRING_OFFSET_BASE;
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[bls12_381::pairing::PRECOMPILE.address().as_slice(), &required_gas.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(required_gas, result_data.into()))
}
