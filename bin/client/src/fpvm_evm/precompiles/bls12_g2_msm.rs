//! Contains the accelerated precompile for the BLS12-381 curve G2 MSM.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g2_msm.rs

use crate::fpvm_evm::precompiles::utils::{msm_required_gas, precompile_run};
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{
    PrecompileError, PrecompileOutput, PrecompileResult, bls12_381,
    bls12_381_const::{DISCOUNT_TABLE_G2_MSM, G2_MSM_BASE_GAS_FEE, G2_MSM_INPUT_LENGTH},
};

/// The maximum input size for the BLS12-381 g2 msm operation after the Isthmus Hardfork.
///
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
const BLS12_MAX_G2_MSM_SIZE_ISTHMUS: usize = 488448;

/// Performs an FPVM-accelerated BLS12-381 G2 msm check.
pub(crate) fn fpvm_bls12_g2_msm<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    let input_len = input.len();

    if input_len > BLS12_MAX_G2_MSM_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "G2MSM input length must be at most {}",
            BLS12_MAX_G2_MSM_SIZE_ISTHMUS
        )));
    }

    if input_len == 0 || input_len % G2_MSM_INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "G2MSM input length should be multiple of {}, was {}",
            G2_MSM_INPUT_LENGTH,
            input_len
        )));
    }

    let k = input_len / G2_MSM_INPUT_LENGTH;
    let required_gas = msm_required_gas(k, &DISCOUNT_TABLE_G2_MSM, G2_MSM_BASE_GAS_FEE);
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[bls12_381::g2_msm::PRECOMPILE.address().as_slice(), &required_gas.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(required_gas, result_data.into()))
}
