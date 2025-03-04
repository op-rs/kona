//! Contains the accelerated version of the KZG point evaluation precompile.

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{Address, Bytes, keccak256};
use revm::{
    precompile::{Error as PrecompileError, PrecompileWithAddress, u64_to_address},
    primitives::{Precompile, PrecompileOutput, PrecompileResult},
};

const POINT_EVAL_ADDRESS: Address = u64_to_address(0x0A);

pub(crate) const FPVM_KZG_POINT_EVAL: PrecompileWithAddress =
    PrecompileWithAddress(POINT_EVAL_ADDRESS, Precompile::Standard(fpvm_kzg_point_eval));

/// Performs an FPVM-accelerated KZG point evaluation precompile call.
fn fpvm_kzg_point_eval(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    const GAS_COST: u64 = 50_000;

    if gas_limit < GAS_COST {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() != 192 {
        return Err(PrecompileError::BlobInvalidInputLength.into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[POINT_EVAL_ADDRESS.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(GAS_COST, result_data.into()))
}
