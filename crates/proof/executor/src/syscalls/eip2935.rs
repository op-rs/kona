//! Contains the [EIP-2935][eip-2935] syscall.
//!
//! [eip-2935]: https://eips.ethereum.org/EIPS/eip-2935

use crate::{
    db::TrieDB,
    errors::{ExecutorError, ExecutorResult},
    syscalls::fill_tx_env_for_contract_call,
    TrieDBProvider,
};
use alloc::boxed::Box;
use alloy_primitives::B256;
use kona_mpt::TrieHinter;
use maili_genesis::RollupConfig;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use revm::{
    db::State,
    primitives::{BlockEnv, CfgEnvWithHandlerCfg, EnvWithHandlerCfg},
    DatabaseCommit, Evm,
};

/// Execute the [EIP-2935][EIP-2935] pre-block block hash contract call.
///
/// If Isthmus is not activated, or the block is the genesis block, then this is a no-op, and no
/// state changes are made.
///
/// Note: this does not commit the state changes to the database, it only transact the call.
///
/// Returns `None` if Isthmus is not active or the block is the genesis block, otherwise returns the
/// result of the call.
///
/// [EIP-2935]: https://eips.ethereum.org/EIPS/eip-2935
pub(crate) fn pre_block_block_hash_contract_call<F, H>(
    db: &mut State<&mut TrieDB<F, H>>,
    config: &RollupConfig,
    block_number: u64,
    initialized_cfg: &CfgEnvWithHandlerCfg,
    initialized_block_env: &BlockEnv,
    parent_block_hash: B256,
    payload: &OpPayloadAttributes,
) -> ExecutorResult<()>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    // apply pre-block EIP-4788 contract call
    let mut evm_pre_block = Evm::builder()
        .with_db(db)
        .with_env_with_handler_cfg(EnvWithHandlerCfg::new_with_cfg_env(
            initialized_cfg.clone(),
            initialized_block_env.clone(),
            Default::default(),
        ))
        .build();

    // initialize a block from the env, because the pre block call needs the block itself
    apply_block_hash_contract_call(
        config,
        payload.payload_attributes.timestamp,
        block_number,
        parent_block_hash,
        &mut evm_pre_block,
    )
}

/// Apply the EIP-4788 pre-block beacon root contract call to a given EVM instance.
fn apply_block_hash_contract_call<F, H>(
    config: &RollupConfig,
    timestamp: u64,
    block_number: u64,
    parent_block_hash: B256,
    evm: &mut Evm<'_, (), &mut State<&mut TrieDB<F, H>>>,
) -> ExecutorResult<()>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    if !config.is_isthmus_active(timestamp) {
        return Ok(());
    }

    // If the block number is zero (genesis block) then no system
    // transaction may occur as per EIP-2935.
    if block_number == 0 {
        return Ok(())
    }

    // Get the previous environment
    let previous_env = Box::new(evm.context.evm.env().clone());

    // modify env for pre block call
    fill_tx_env_for_contract_call(
        &mut evm.context.evm.env,
        alloy_eips::eip4788::SYSTEM_ADDRESS,
        alloy_eips::eip2935::HISTORY_STORAGE_ADDRESS,
        parent_block_hash.into(),
    );

    let mut state = match evm.transact() {
        Ok(res) => res.state,
        Err(e) => {
            evm.context.evm.env = previous_env;
            return Err(ExecutorError::ExecutionError(e));
        }
    };

    // NOTE: Revm currently marks these accounts as "touched" when we do the above transact calls,
    // and includes them in the result.
    //
    // There should be no state changes to these addresses anyways as a result of this system call,
    // so we can just remove them from the state returned.
    state.remove(&alloy_eips::eip4788::SYSTEM_ADDRESS);
    state.remove(&evm.block().coinbase);

    evm.context.evm.db.commit(state);

    // re-set the previous env
    evm.context.evm.env = previous_env;

    Ok(())
}
