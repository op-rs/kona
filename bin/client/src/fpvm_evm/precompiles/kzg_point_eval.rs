//! Contains the accelerated version of the KZG point evaluation precompile.

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use alloy_primitives::Address;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{PrecompileError, PrecompileOutput, PrecompileResult};

/// Address of the KZG point evaluation precompile.
pub(crate) const KZG_POINT_EVAL_ADDR: Address = revm::precompile::u64_to_address(0x0A);

/// Runs the FPVM-accelerated `kzgPointEval` precompile call.
pub(crate) fn fpvm_kzg_point_eval<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    const GAS_COST: u64 = 50_000;

    if gas_limit < GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    if input.len() != 192 {
        return Err(PrecompileError::BlobInvalidInputLength);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[KZG_POINT_EVAL_ADDR.as_slice(), &GAS_COST.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(GAS_COST, result_data.into()))
}
