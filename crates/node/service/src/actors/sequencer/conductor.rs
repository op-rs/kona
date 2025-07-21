use alloy_rpc_client::ReqwestClient;
use alloy_transport::{RpcError, TransportErrorKind};
use url::Url;

/// A client for communicating with the conductor service via RPC
#[derive(Debug, Clone)]
pub struct ConductorClient {
    /// The inner RPC provider
    rpc: ReqwestClient,
}

impl ConductorClient {
    /// Creates a new conductor client using HTTP transport
    pub fn new_http(url: Url) -> Self {
        let rpc = ReqwestClient::new_http(url);
        Self { rpc }
    }

    /// Check if the node is a leader of the conductor.
    pub async fn leader(&self) -> Result<bool, ConductorError> {
        let result: bool = self.rpc.request("conductor_leader", ()).await?;
        Ok(result)
    }

    /// Check if the conductor is active.
    pub async fn conductor_active(&self) -> Result<bool, ConductorError> {
        let result: bool = self.rpc.request("conductor_active", ()).await?;
        Ok(result)
    }

    /// Override the leader of the conductor.
    pub async fn override_leader(&self) -> Result<(), ConductorError> {
        let _result: () = self.rpc.request("conductor_overrideLeader", ()).await?;
        Ok(())
    }
}

/// Error type for conductor operations
#[derive(Debug, thiserror::Error)]
pub enum ConductorError {
    /// An error occurred while making an RPC call to the conductor.
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError<TransportErrorKind>),
}
