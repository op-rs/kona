//! Contains the accelerated version of the `ecrecover` precompile.

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{Address, Bytes, keccak256};
use revm::{
    precompile::{Error as PrecompileError, PrecompileWithAddress, u64_to_address},
    primitives::{Precompile, PrecompileOutput, PrecompileResult},
};

const ECRECOVER_ADDRESS: Address = u64_to_address(1);

pub(crate) const FPVM_ECRECOVER: PrecompileWithAddress =
    PrecompileWithAddress(ECRECOVER_ADDRESS, Precompile::Standard(fpvm_ecrecover));

/// Performs an FPVM-accelerated `ecrecover` precompile call.
fn fpvm_ecrecover(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    const ECRECOVER_BASE: u64 = 3_000;

    if ECRECOVER_BASE > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[ECRECOVER_ADDRESS.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(ECRECOVER_BASE, result_data.into()))
}
