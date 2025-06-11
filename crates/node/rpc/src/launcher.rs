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
    /// Failed to register a method on the [`RpcModule`].
    #[error("failed to register method: {0}")]
    RegisterMethod(#[from] RegisterMethodError),
}

impl PartialEq for RpcLauncherError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MissingSocket, Self::MissingSocket) => true,
            (Self::ServerStart(e1), Self::ServerStart(e2)) => e1.kind() == e2.kind(),
            _ => false,
        }
    }
}

/// A healthcheck response for the RPC server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HealthzResponse {
    /// The application version.
    version: String,
}

/// Launches a [`Server`] using a set of [`RpcModule`]s.
#[derive(Debug, Clone, Default)]
pub struct RpcLauncher {
    disabled: bool,
    /// If `true`, the RPC server will not attempt to restart if it stops.
    pub no_restart: bool,
    socket: Option<SocketAddr>,
    module: Option<RpcModule<()>>,
    /// Enable the websocket rpc server
    pub ws_enabled: bool,
}

impl From<SocketAddr> for RpcLauncher {
    fn from(socket: SocketAddr) -> Self {
        Self {
            disabled: false,
            no_restart: false,
            socket: Some(socket),
            module: None,
            ws_enabled: false,
        }
    }
}

impl RpcLauncher {
    /// Creates a new [`RpcLauncher`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable the RPC server, preventing the launcher from starting the RPC server.
    pub const fn disable(&mut self) {
        self.disabled = true;
    }

    /// Returns whether WebSocket RPC endpoint is enabled
    pub const fn ws_enabled(&self) -> bool {
        self.ws_enabled
    }

    /// Merges a given [`RpcModule`] into the [`RpcLauncher`].
    pub fn merge<CTX>(
        mut self,
        module: Option<RpcModule<CTX>>,
    ) -> Result<Self, RegisterMethodError> {
        let Some(module) = module else {
            return Ok(self);
        };
        let mut existing = self.module.take().map_or_else(|| RpcModule::new(()), |m| m);
        existing.merge(module)?;
        Ok(Self { module: Some(existing), ..self })
    }

    /// Sets the given [`SocketAddr`] on the [`RpcLauncher`].
    pub fn set_addr(self, addr: SocketAddr) -> Self {
        Self { socket: Some(addr), ..self }
    }

    /// Launches the jsonrpsee [`Server`].
    ///
    /// If the RPC server is disabled, this will return `Ok(None)`.
    ///
    /// The `/healthz` endpoint is automatically merged into the RPC module.
    ///
    /// ## Errors
    ///
    /// - [`RpcLauncherError::MissingSocket`] if the socket address is missing.
    /// - [`RpcLauncherError::ServerStart`] if the server fails to start.
    pub async fn launch(&mut self) -> Result<Option<ServerHandle>, RpcLauncherError> {
        if self.disabled {
            return Ok(None);
        }
        let socket = self.socket.take().ok_or(RpcLauncherError::MissingSocket)?;
        let server = Server::builder().build(socket).await?;
        let mut module = self.module.take().unwrap_or_else(|| RpcModule::new(()));
        module.register_method("healthz", |_, _, _| {
            let response = HealthzResponse { version: std::env!("CARGO_PKG_VERSION").to_string() };
            jsonrpsee::core::RpcResult::Ok(response)
        })?;
        Ok(Some(server.start(module)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_launch_missing_socket() {
        let mut launcher = RpcLauncher::new();
        let result = launcher.launch().await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RpcLauncherError::MissingSocket);
    }

    #[tokio::test]
    async fn test_launch_no_modules() {
        let mut launcher = RpcLauncher::new();
        launcher = launcher.set_addr(SocketAddr::from(([127, 0, 0, 1], 8080)));
        let result = launcher.launch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_launch_with_modules() {
        let mut launcher = RpcLauncher::new();
        launcher = launcher.set_addr(SocketAddr::from(([127, 0, 0, 1], 8081)));
        launcher = launcher.merge(Some(RpcModule::new(()))).expect("module merge");
        launcher = launcher.merge::<()>(None).expect("module merge");
        launcher = launcher.merge(Some(RpcModule::new(()))).expect("module merge");
        let result = launcher.launch().await;
        assert!(result.is_ok());
    }
}
