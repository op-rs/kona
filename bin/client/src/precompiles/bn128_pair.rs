//! Contains the accelerated version of the `ecPairing` precompile.

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{Address, Bytes, keccak256};
use revm::{
    precompile::{
        Error as PrecompileError, PrecompileWithAddress,
        bn128::pair::{ISTANBUL_PAIR_BASE, ISTANBUL_PAIR_PER_POINT},
        u64_to_address,
    },
    primitives::{Precompile, PrecompileOutput, PrecompileResult},
};

const ECPAIRING_ADDRESS: Address = u64_to_address(8);
const PAIR_ELEMENT_LEN: usize = 64 + 128;

pub(crate) const FPVM_ECPAIRING: PrecompileWithAddress =
    PrecompileWithAddress(ECPAIRING_ADDRESS, Precompile::Standard(fpvm_ecpairing));

pub(crate) const FPVM_ECPAIRING_GRANITE: PrecompileWithAddress =
    PrecompileWithAddress(ECPAIRING_ADDRESS, Precompile::Standard(fpvm_ecpairing_granite));

/// Performs an FPVM-accelerated `ecpairing` precompile call.
fn fpvm_ecpairing(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let gas_used =
        (input.len() / PAIR_ELEMENT_LEN) as u64 * ISTANBUL_PAIR_PER_POINT + ISTANBUL_PAIR_BASE;

    if gas_used > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() % PAIR_ELEMENT_LEN != 0 {
        return Err(PrecompileError::Bn128PairLength.into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[ECPAIRING_ADDRESS.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(gas_used, result_data.into()))
}

/// Performs an FPVM-accelerated `ecpairing` precompile call after the Granite hardfork.
fn fpvm_ecpairing_granite(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    const BN256_MAX_PAIRING_SIZE_GRANITE: usize = 112_687;
    if input.len() > BN256_MAX_PAIRING_SIZE_GRANITE {
        return Err(PrecompileError::Bn128PairLength.into());
    }

    fpvm_ecpairing(input, gas_limit)
}
