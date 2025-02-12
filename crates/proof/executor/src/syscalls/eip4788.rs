//! Contains the logic for executing the pre-block beacon root call.

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

/// Execute the EIP-4788 pre-block beacon root contract call.
pub(crate) fn pre_block_beacon_root_contract_call<F, H>(
    db: &mut State<&mut TrieDB<F, H>>,
    config: &RollupConfig,
    block_number: u64,
    initialized_cfg: &CfgEnvWithHandlerCfg,
    initialized_block_env: &BlockEnv,
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
    apply_beacon_root_contract_call(
        config,
        payload.payload_attributes.timestamp,
        block_number,
        payload.payload_attributes.parent_beacon_block_root,
        &mut evm_pre_block,
    )
}

/// Apply the EIP-4788 pre-block beacon root contract call to a given EVM instance.
fn apply_beacon_root_contract_call<F, H>(
    config: &RollupConfig,
    timestamp: u64,
    block_number: u64,
    parent_beacon_block_root: Option<B256>,
    evm: &mut Evm<'_, (), &mut State<&mut TrieDB<F, H>>>,
) -> ExecutorResult<()>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    if !config.is_ecotone_active(timestamp) {
        return Ok(());
    }

    let parent_beacon_block_root =
        parent_beacon_block_root.ok_or(ExecutorError::MissingParentBeaconBlockRoot)?;

    // if the block number is zero (genesis block) then the parent beacon block root must
    // be 0x0 and no system transaction may occur as per EIP-4788
    if block_number == 0 {
        if parent_beacon_block_root != B256::ZERO {
            return Err(ExecutorError::MissingParentBeaconBlockRoot);
        }
        return Ok(());
    }

    // Get the previous environment
    let previous_env = Box::new(evm.context.evm.env().clone());

    // modify env for pre block call
    fill_tx_env_for_contract_call(
        &mut evm.context.evm.env,
        alloy_eips::eip4788::SYSTEM_ADDRESS,
        alloy_eips::eip4788::BEACON_ROOTS_ADDRESS,
        parent_beacon_block_root.0.into(),
    );

    let mut state = match evm.transact() {
        Ok(res) => res.state,
        Err(e) => {
            evm.context.evm.env = previous_env;
            return Err(ExecutorError::ExecutionError(e));
        }
    };

    state.remove(&alloy_eips::eip4788::SYSTEM_ADDRESS);
    state.remove(&evm.block().coinbase);

    evm.context.evm.db.commit(state);

    // re-set the previous env
    evm.context.evm.env = previous_env;

    Ok(())
}
