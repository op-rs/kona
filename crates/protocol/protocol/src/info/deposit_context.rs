//! Contains the logic for creating a deposit context closing transaction for the interop hardfork.

use super::L1BlockInfoTx;
use crate::info::variant::{L1_BLOCK_ADDRESS, L1_INFO_DEPOSITOR_ADDRESS};
use alloy_consensus::Sealed;
use alloy_primitives::{Sealable, TxKind, U256};
use op_alloy_consensus::{DepositContextDepositSource, DepositSourceDomain, TxDeposit};

/// `keccak256("depositsComplete()")[4:]`
const DEPOSIT_CONTEXT_SELECTOR: [u8; 4] = [0xe3, 0x2d, 0x20, 0xbb];

/// Create a [TxDeposit] for closing the deposit context. This deposit transaction, after
/// interop activation, always is placed last in the deposit section of the block.
///
/// <https://specs.optimism.io/interop/derivation.html#closing-the-deposit-context>
pub fn closing_deposit_context_tx(
    l1_info: &L1BlockInfoTx,
    sequence_number: u64,
) -> Sealed<TxDeposit> {
    let source = DepositSourceDomain::DepositContext(DepositContextDepositSource {
        l1_block_hash: l1_info.block_hash(),
        seq_number: sequence_number,
    });

    let deposit_tx = TxDeposit {
        source_hash: source.source_hash(),
        from: L1_INFO_DEPOSITOR_ADDRESS,
        to: TxKind::Call(L1_BLOCK_ADDRESS),
        mint: None,
        value: U256::ZERO,
        gas_limit: 36_000,
        is_system_transaction: false,
        input: DEPOSIT_CONTEXT_SELECTOR.into(),
    };

    deposit_tx.seal_slow()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::L1BlockInfoBedrock;
    use alloy_primitives::{Address, Bytes, B256};

    #[test]
    fn test_closing_deposit() {
        let bedrock_info_tx = L1BlockInfoBedrock {
            number: 1,
            time: 2,
            base_fee: 3,
            block_hash: B256::from([4; 32]),
            sequence_number: 5,
            batcher_address: Address::from([6; 20]),
            l1_fee_overhead: U256::from(7),
            l1_fee_scalar: U256::from(8),
        };
        let l1_info = L1BlockInfoTx::Bedrock(bedrock_info_tx);

        let sequence_number = 9;
        let deposit_tx = closing_deposit_context_tx(&l1_info, sequence_number);

        assert_eq!(deposit_tx.from, L1_INFO_DEPOSITOR_ADDRESS);
        assert_eq!(deposit_tx.to, TxKind::Call(L1_BLOCK_ADDRESS));
        assert_eq!(deposit_tx.value, U256::ZERO);
        assert_eq!(deposit_tx.gas_limit, 36_000);
        assert!(!deposit_tx.is_system_transaction);
        assert_eq!(deposit_tx.input, Bytes::from(DEPOSIT_CONTEXT_SELECTOR));
    }
}
