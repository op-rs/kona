//! Contains the accelerated precompile for the BLS12-381 curve G1 Point Addition.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g1_add.rs

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{PrecompileError, PrecompileOutput, PrecompileResult, bls12_381};

/// Input length of G1 Addition operation.
const INPUT_LENGTH: usize = 256;

/// Base gas fee for the BLS12-381 g1 addition operation.
const G1_ADD_BASE_FEE: u64 = 375;

/// Performs an FPVM-accelerated BLS12-381 G1 addition check.
///
/// Notice, there is no input size limit for this precompile.
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
pub(crate) fn fpvm_bls12_g1_add<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    if G1_ADD_BASE_FEE > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let input_len = input.len();
    if input_len != INPUT_LENGTH {
        return Err(PrecompileError::Other(alloc::format!(
            "G1 addition input length should be multiple of {INPUT_LENGTH}, was {input_len}"
        )));
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[bls12_381::g1_add::PRECOMPILE.address().as_slice(), &G1_ADD_BASE_FEE.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(G1_ADD_BASE_FEE, result_data.into()))
}
