use alloy_primitives::B256;
use async_trait::async_trait;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use crate::actors::sequencer::rpc::{QueuedRequestError, SequencerRpcError};
use crate::ConductorError;

#[async_trait]
pub trait SequencerAdminAPI {
    async fn is_sequencer_active(&self) -> Result<bool, SequencerAdminAPIError>;

    async fn is_conductor_enabled(&self) -> Result<bool, SequencerAdminAPIError>;

    async fn start_sequencer(&self) -> Result<(), SequencerAdminAPIError>;

    async fn stop_sequencer(&self) -> Result<B256, SequencerAdminAPIError>;

    async fn set_recovery_mode(&self, mode: bool) -> Result<(), SequencerAdminAPIError>;

    async fn override_leader(&self) -> Result<(), SequencerAdminAPIError>;
}

#[derive(Debug, Error)]
pub enum SequencerAdminAPIError {
    /// An error sending the request or receiving the response.
    #[error(transparent)]
    CommunicationError(#[from] QueuedRequestError),

    LeaderOverrideError(#[from] ConductorError),
}



#[async_trait]
pub trait SequencerAdminAPIServer {
    async fn serve(&mut self, cancellation_token: CancellationToken) -> Result<(), SequencerAdminAPIServerError>;
}

#[derive(Debug, Error)]
pub enum SequencerAdminAPIServerError {
    /// A fatal error occurred.
    #[error("A fatal error occurred")]
    Fatal,
}