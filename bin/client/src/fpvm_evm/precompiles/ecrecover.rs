//! Contains the accelerated version of the `ecrecover` precompile.

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use alloy_primitives::Address;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{PrecompileError, PrecompileOutput, PrecompileResult};

/// Address of the `ecrecover` precompile.
pub(crate) const ECRECOVER_ADDR: Address = revm::precompile::u64_to_address(1);

/// Runs the FPVM-accelerated `ecrecover` precompile call.
pub(crate) fn fpvm_ec_recover<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    const ECRECOVER_BASE: u64 = 3_000;

    if ECRECOVER_BASE > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[ECRECOVER_ADDR.as_slice(), &ECRECOVER_BASE.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))
    .unwrap_or_default();

    Ok(PrecompileOutput::new(ECRECOVER_BASE, result_data.into()))
}
