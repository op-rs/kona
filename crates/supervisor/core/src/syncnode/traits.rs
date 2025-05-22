use thiserror::Error;
use crate::types::{BlockRef, ManagedEvent};

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("JWT error: {0}")]
    JwtError(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("PubSub error: {0}")]
    PubSub(String),
    #[error("Attaching Node error: {0}")]
    AttachNodeError(String),
}

trait SyncSource<RpcError> {
    fn chain_id() -> Result<u64, RpcError>;
}

trait SyncControl<RpcError> {
    fn subscribe_events(event: ManagedEvent) -> Result<(), RpcError>;
    fn provide_l1(next_l1: BlockRef) -> Result<(), RpcError>;
}

trait SyncNode: SyncSource<RpcError> + SyncControl<RpcError> {}
