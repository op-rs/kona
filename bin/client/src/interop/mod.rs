//! Multi-chain, interoperable fault proof program entrypoint.

use alloc::sync::Arc;
use alloy_primitives::B256;
use consolidate::consolidate_dependencies;
use core::fmt::Debug;
use kona_driver::DriverError;
use kona_executor::{ExecutorError, KonaHandleRegister};
use kona_preimage::{HintWriterClient, PreimageOracleClient};
use kona_proof::{errors::OracleProviderError, l2::OracleL2ChainProvider, CachingOracle};
use kona_proof_interop::{
    boot::BootstrapError, BootInfo, ConsolidationError, PreState, TRANSITION_STATE_MAX_STEPS,
};
use thiserror::Error;
use tracing::{error, info};
use transition::sub_transition;

pub(crate) mod consolidate;
pub(crate) mod transition;
pub(crate) mod util;

/// An error that can occur when running the fault proof program.
#[derive(Error, Debug)]
pub enum FaultProofProgramError {
    /// The claim is invalid.
    #[error("Invalid claim. Expected {0}, actual {1}")]
    InvalidClaim(B256, B256),
    /// An error occurred in the Oracle provider.
    #[error(transparent)]
    OracleProvider(#[from] OracleProviderError),
    /// An error occurred in the driver.
    #[error(transparent)]
    Driver(#[from] DriverError<ExecutorError>),
    /// Consolidation error.
    #[error(transparent)]
    Consolidation(#[from] ConsolidationError),
    /// Bootstrap error
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
    /// State transition failed.
    #[error("Critical state transition failure")]
    StateTransitionFailed,
    /// Missing a rollup configuration.
    #[error("Missing rollup configuration for chain ID {0}")]
    MissingRollupConfig(u64),
}

/// Executes the interop fault proof program with the given [PreimageOracleClient] and
/// [HintWriterClient].
#[inline]
pub async fn run<P, H>(
    oracle_client: P,
    hint_client: H,
    handle_register: Option<
        KonaHandleRegister<
            OracleL2ChainProvider<CachingOracle<P, H>>,
            OracleL2ChainProvider<CachingOracle<P, H>>,
        >,
    >,
) -> Result<(), FaultProofProgramError>
where
    P: PreimageOracleClient + Send + Sync + Debug + Clone,
    H: HintWriterClient + Send + Sync + Debug + Clone,
{
    const ORACLE_LRU_SIZE: usize = 1024;

    // Instantiate the oracle and bootstrap the program from local inputs.
    let oracle = Arc::new(CachingOracle::new(ORACLE_LRU_SIZE, oracle_client, hint_client));
    let boot = match BootInfo::load(oracle.as_ref()).await {
        Ok(boot) => boot,
        Err(BootstrapError::NoOpTransition) => {
            info!(target: "client_interop", "No-op transition, short-circuiting.");
            return Ok(());
        }
        Err(e) => {
            error!(target: "client_interop", "Failed to load boot info: {}", e);
            return Err(e.into());
        }
    };

    // Load in the agreed pre-state from the preimage oracle in order to determine the active
    // sub-problem.
    match boot.agreed_pre_state {
        PreState::SuperRoot(_) => {
            // If the pre-state is a super root, the first sub-problem is always selected.
            sub_transition(oracle, handle_register, boot).await
        }
        PreState::TransitionState(ref transition_state) => {
            // If the pre-state is a transition state, the sub-problem is selected based on the
            // current step.
            if transition_state.step < TRANSITION_STATE_MAX_STEPS {
                sub_transition(oracle, handle_register, boot).await
            } else {
                consolidate_dependencies(oracle, boot).await
            }
        }
    }
}
