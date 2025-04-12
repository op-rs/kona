//! Contains the accelerated precompile for the BLS12-381 curve G1 MSM.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g1_msm.rs

use crate::fpvm_evm::precompiles::utils::{msm_required_gas, precompile_run};
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{
    PrecompileError, PrecompileOutput, PrecompileResult, bls12_381,
    bls12_381_const::{DISCOUNT_TABLE_G1_MSM, G1_MSM_BASE_GAS_FEE, G1_MSM_INPUT_LENGTH},
};

/// The maximum input size for the BLS12-381 g1 msm operation after the Isthmus Hardfork.
///
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
const BLS12_MAX_G1_MSM_SIZE_ISTHMUS: usize = 513760;

/// Performs an FPVM-accelerated `bls12` g1 msm check precompile call after the Isthmus Hardfork.
pub(crate) fn fpvm_bls12_g1_msm<C: Channel + Send + Sync>(
    input: &[u8],
    gas_limit: u64,
    hint_writer: &HintWriter<C>,
    oracle_reader: &OracleReader<C>,
) -> PrecompileResult {
    if input.len() > BLS12_MAX_G1_MSM_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "G1MSM input length must be at most {}",
            BLS12_MAX_G1_MSM_SIZE_ISTHMUS
        )));
    }

    let input_len = input.len();
    if input_len == 0 || input_len % G1_MSM_INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "G1MSM input length should be multiple of {}, was {}",
            G1_MSM_INPUT_LENGTH,
            input_len
        )));
    }

    let k = input_len / G1_MSM_INPUT_LENGTH;
    let required_gas = msm_required_gas(k, &DISCOUNT_TABLE_G1_MSM, G1_MSM_BASE_GAS_FEE);
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let result_data = kona_proof::block_on(precompile_run! {
        hint_writer,
        oracle_reader,
        &[bls12_381::g1_msm::PRECOMPILE.address().as_slice(), &required_gas.to_be_bytes(), input]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(required_gas, result_data.into()))
}

#[cfg(test)]
mod test {
    use alloy_primitives::hex;

    use super::*;
    use crate::fpvm_evm::precompiles::test_utils::{
        execute_native_precompile, test_accelerated_precompile,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bls12_381_g1_msm() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            // https://raw.githubusercontent.com/ethereum/execution-spec-tests/a1c4eeff347a64ad6c5aedd51314d4ffc067346b/tests/prague/eip2537_bls_12_381_precompiles/vectors/msm_G1_bls.json
            let input = hex!("0000000000000000000000000000000017f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb0000000000000000000000000000000008b3f481e3aaa0f1a09e30ed741d8ae4fcf5e095d5d00af600db18cb2c04b3edd03cc744a2888ae40caa232946c5e7e10000000000000000000000000000000000000000000000000000000000000002");
            let expected = hex!("000000000000000000000000000000000572cbea904d67468808c8eb50a9450c9721db309128012543902d0ac358a62ae28f75bb8f1c7c42c39a8c5529bf0f4e00000000000000000000000000000000166a9d8cabc673a322fda673779d8e3822ba3ecb8670e461f73bb9021d5fd76a4c56d9d4cd16bd1bba86881979749d28");

            let accelerated_result = fpvm_bls12_g1_msm(&input, 12000, hint_writer, oracle_reader).unwrap();
            let native_result = execute_native_precompile(*bls12_381::g1_msm::PRECOMPILE.address(), input, 12000).unwrap();

            assert_eq!(accelerated_result.bytes.as_ref(), expected.as_ref());
            assert_eq!(accelerated_result.bytes, native_result.bytes);
            assert_eq!(accelerated_result.gas_used, native_result.gas_used);
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bls12_381_g1_msm_bad_input_len_isthmus() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let accelerated_result = fpvm_bls12_g1_msm(
                &[0u8; BLS12_MAX_G1_MSM_SIZE_ISTHMUS + 1],
                u64::MAX,
                hint_writer,
                oracle_reader,
            )
            .unwrap_err();
            assert!(matches!(accelerated_result, PrecompileError::Other(_)));
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bls12_381_g1_msm_bad_input_len() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let accelerated_result =
                fpvm_bls12_g1_msm(&[], u64::MAX, hint_writer, oracle_reader).unwrap_err();
            assert!(matches!(accelerated_result, PrecompileError::Other(_)));
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bls12_381_g1_msm_bad_gas_limit() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let accelerated_result =
                fpvm_bls12_g1_msm(&[0u8; G1_MSM_INPUT_LENGTH], 0, hint_writer, oracle_reader)
                    .unwrap_err();
            assert!(matches!(accelerated_result, PrecompileError::OutOfGas));
        })
        .await;
    }
}
