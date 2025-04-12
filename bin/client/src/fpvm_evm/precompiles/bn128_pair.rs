//! Contains the accelerated version of the `ecPairing` precompile.

use crate::fpvm_evm::precompiles::utils::precompile_run;
use alloc::string::ToString;
use kona_preimage::{Channel, HintWriter, OracleReader};
use revm::precompile::{
    PrecompileError, PrecompileOutput, PrecompileResult,
    bn128::{
        PAIR_ELEMENT_LEN,
        pair::{self, ISTANBUL_PAIR_BASE, ISTANBUL_PAIR_PER_POINT},
    },
};

const BN256_MAX_PAIRING_SIZE_GRANITE: usize = 112_687;

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
    if input.len() > BN256_MAX_PAIRING_SIZE_GRANITE {
        return Err(PrecompileError::Bn128PairLength);
    }

    fpvm_bn128_pair(input, gas_limit, hint_writer, oracle_reader)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fpvm_evm::precompiles::test_utils::{
        execute_native_precompile, test_accelerated_precompile,
    };
    use alloy_primitives::hex;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bn128_pairing() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            const EXPECTED_RESULT: [u8; 32] = hex!("0000000000000000000000000000000000000000000000000000000000000001");

            let input = hex!(
                "2cf44499d5d27bb186308b7af7af02ac5bc9eeb6a3d147c186b21fb1b76e18da2c0f001f52110ccfe69108924926e45f0b0c868df0e7bde1fe16d3242dc715f61fb19bb476f6b9e44e2a32234da8212f61cd63919354bc06aef31e3cfaff3ebc22606845ff186793914e03e21df544c34ffe2f2f3504de8a79d9159eca2d98d92bd368e28381e8eccb5fa81fc26cf3f048eea9abfdd85d7ed3ab3698d63e4f902fe02e47887507adf0ff1743cbac6ba291e66f59be6bd763950bb16041a0a85e000000000000000000000000000000000000000000000000000000000000000130644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd451971ff0471b09fa93caaf13cbf443c1aede09cc4328f5a62aad45f40ec133eb4091058a3141822985733cbdddfed0fd8d6c104e9e9eff40bf5abfef9ab163bc72a23af9a5ce2ba2796c1f4e453a370eb0af8c212d9dc9acd8fc02c2e907baea223a8eb0b0996252cb548a4487da97b02422ebc0e834613f954de6c7e0afdc1fc"
            );
            let accelerated_result = fpvm_bn128_pair(&input, u64::MAX, hint_writer, oracle_reader).unwrap();
            let native_result = execute_native_precompile(*pair::ISTANBUL.address(), input, u64::MAX).unwrap();

            assert_eq!(accelerated_result.bytes.as_ref(), EXPECTED_RESULT.as_ref());
            assert_eq!(accelerated_result.bytes, native_result.bytes);
            assert_eq!(accelerated_result.gas_used, native_result.gas_used);
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bn128_pairing_granite() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            const EXPECTED_RESULT: [u8; 32] = hex!("0000000000000000000000000000000000000000000000000000000000000001");

            let input = hex!(
                "2cf44499d5d27bb186308b7af7af02ac5bc9eeb6a3d147c186b21fb1b76e18da2c0f001f52110ccfe69108924926e45f0b0c868df0e7bde1fe16d3242dc715f61fb19bb476f6b9e44e2a32234da8212f61cd63919354bc06aef31e3cfaff3ebc22606845ff186793914e03e21df544c34ffe2f2f3504de8a79d9159eca2d98d92bd368e28381e8eccb5fa81fc26cf3f048eea9abfdd85d7ed3ab3698d63e4f902fe02e47887507adf0ff1743cbac6ba291e66f59be6bd763950bb16041a0a85e000000000000000000000000000000000000000000000000000000000000000130644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd451971ff0471b09fa93caaf13cbf443c1aede09cc4328f5a62aad45f40ec133eb4091058a3141822985733cbdddfed0fd8d6c104e9e9eff40bf5abfef9ab163bc72a23af9a5ce2ba2796c1f4e453a370eb0af8c212d9dc9acd8fc02c2e907baea223a8eb0b0996252cb548a4487da97b02422ebc0e834613f954de6c7e0afdc1fc"
            );
            let accelerated_result = fpvm_bn128_pair_granite(&input, u64::MAX, hint_writer, oracle_reader).unwrap();
            let native_result = execute_native_precompile(*pair::ISTANBUL.address(), input, u64::MAX).unwrap();

            assert_eq!(accelerated_result.bytes.as_ref(), EXPECTED_RESULT.as_ref());
            assert_eq!(accelerated_result.bytes, native_result.bytes);
            assert_eq!(accelerated_result.gas_used, native_result.gas_used);
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bn128_pairing_not_enough_gas() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let input = hex!("0badc0de");
            let accelerated_result =
                fpvm_bn128_pair(&input, 0, hint_writer, oracle_reader).unwrap_err();

            assert!(matches!(accelerated_result, PrecompileError::OutOfGas));
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bn128_pairing_bad_input_len() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let input = hex!("0badc0de");
            let accelerated_result =
                fpvm_bn128_pair(&input, u64::MAX, hint_writer, oracle_reader).unwrap_err();

            assert!(matches!(accelerated_result, PrecompileError::Bn128PairLength));
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_accelerated_bn128_pairing_bad_input_len_granite() {
        test_accelerated_precompile(|hint_writer, oracle_reader| {
            let input = [0u8; BN256_MAX_PAIRING_SIZE_GRANITE + 1];
            let accelerated_result =
                fpvm_bn128_pair(&input, u64::MAX, hint_writer, oracle_reader).unwrap_err();

            assert!(matches!(accelerated_result, PrecompileError::Bn128PairLength));
        })
        .await;
    }
}
