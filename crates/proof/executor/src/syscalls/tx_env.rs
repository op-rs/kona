//! Contains a helper function to fill the [`TxEnv`] for the given system call.

use alloc::vec::Vec;
use alloy_primitives::{Address, Bytes, U256};
use revm::primitives::{Env, OptimismFields, TransactTo, TxEnv};

/// Fill transaction environment with the system caller and the system
/// contract address and message data.
///
/// This is a system operation and therefore:
///  * the call must execute to completion
///  * the call does not count against the block’s gas limit
///  * the call does not follow the EIP-1559 burn semantics - no value should be transferred as part
///    of the call
///  * if no code exists at the provided address, the call will fail silently
pub(crate) fn fill_tx_env_for_contract_call(
    env: &mut Env,
    caller: Address,
    contract: Address,
    data: Bytes,
) {
    env.tx = TxEnv {
        caller,
        transact_to: TransactTo::Call(contract),
        // Explicitly set nonce to None so revm does not do any nonce checks
        nonce: None,
        gas_limit: 30_000_000,
        value: U256::ZERO,
        data,
        // Setting the gas price to zero enforces that no value is transferred as part of the call,
        // and that the call will not count against the block's gas limit
        gas_price: U256::ZERO,
        // The chain ID check is not relevant here and is disabled if set to None
        chain_id: None,
        // Setting the gas priority fee to None ensures the effective gas price is derived from the
        // `gas_price` field, which we need to be zero
        gas_priority_fee: None,
        access_list: Vec::new(),
        authorization_list: None,
        // blob fields can be None for this tx
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: None,
        optimism: OptimismFields {
            source_hash: None,
            mint: None,
            is_system_transaction: Some(false),
            // The L1 fee is not charged for the EIP-4788 transaction, submit zero bytes for the
            // enveloped tx size.
            enveloped_tx: Some(Bytes::default()),
        },
    };

    // ensure the block gas limit is >= the tx
    env.block.gas_limit = U256::from(env.tx.gas_limit);

    // disable the base fee check for this call by setting the base fee to zero
    env.block.basefee = U256::ZERO;
}
