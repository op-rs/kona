//! Contains the [`RpcLauncher`] service.

use jsonrpsee::server::{RegisterMethodError, RpcModule, Server, ServerHandle};
use std::net::SocketAddr;

/// An error that can occur when using the [`RpcLauncher`].
#[derive(Debug, thiserror::Error)]
pub enum RpcLauncherError {
    /// The [`SocketAddr`] is missing.
    #[error("socket address is missing")]
    MissingSocket,
    /// An error occurred while starting the [`Server`].
    #[error("failed to start server: {0}")]
    ServerStart(#[from] std::io::Error),
}

/// Launches a [`Server`] using a set of [`RpcModule`]s.
#[derive(Debug, Clone, Default)]
pub struct RpcLauncher {
    socket: Option<SocketAddr>,
    module: Option<RpcModule<()>>,
}

impl RpcLauncher {
    /// Creates a new [`RpcLauncher`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Merges a given [`RpcModule`] into the [`RpcLauncher`].
    pub fn merge<CTX>(mut self, module: RpcModule<CTX>) -> Result<Self, RegisterMethodError> {
        let mut existing = self.module.take().map_or_else(|| RpcModule::new(()), |m| m);
        existing.merge(module)?;
        Ok(Self { module: Some(existing), ..self })
    }

    /// Sets the given [`SocketAddr`] on the [`RpcLauncher`].
    pub fn set_addr(self, addr: SocketAddr) -> Self {
        Self { socket: Some(addr), ..self }
    }

    /// Starts the [`Server`].
    pub async fn start(&mut self) -> Result<ServerHandle, RpcLauncherError> {
        let socket = self.socket.take().ok_or(RpcLauncherError::MissingSocket)?;
        let server = Server::builder().build(socket).await?;
        let module = self.module.take().unwrap_or_else(|| RpcModule::new(()));
        Ok(server.start(module))
    }
}
