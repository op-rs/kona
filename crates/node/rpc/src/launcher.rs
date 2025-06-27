//! Contains the [`RpcBuilder`] service.

use jsonrpsee::server::{RegisterMethodError, RpcModule, Server, ServerHandle};
use std::net::SocketAddr;

use crate::RpcConfig;

/// An error that can occur when using the [`RpcBuilder`].
#[derive(Debug, thiserror::Error)]
pub enum RpcLauncherError {
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
#[derive(Debug, Clone)]
pub struct RpcBuilder {
    /// The RPC configuration associated with the [`RpcBuilder`].
    pub(crate) config: RpcConfig,
    /// The modules to register on the RPC server.
    pub(crate) module: RpcModule<()>,
}

impl RpcBuilder {
    /// Creates a new [`RpcBuilder`].
    pub fn new(config: RpcConfig) -> Self {
        Self { config, module: RpcModule::new(()) }
    }

    /// Creates a new [`RpcBuilder`] that is disabled.
    pub fn new_disabled() -> Self {
        Self {
            config: RpcConfig {
                disabled: true,
                no_restart: false,
                // Use a dummy socket address. The RPC server is disabled, so it will not be used.
                socket: SocketAddr::from(([127, 0, 0, 1], 8080)),
                enable_admin: false,
                admin_persistence: None,
                ws_enabled: false,
            },
            module: RpcModule::new(()),
        }
    }

    /// Returns whether WebSocket RPC endpoint is enabled
    pub const fn ws_enabled(&self) -> bool {
        self.config.ws_enabled
    }

    /// Merges a given [`RpcModule`] into the [`RpcBuilder`].
    pub fn merge<CTX>(&mut self, other: RpcModule<CTX>) -> Result<(), RegisterMethodError> {
        self.module.merge(other)?;

        Ok(())
    }

    /// Returns the socket address of the [`RpcBuilder`].
    pub const fn socket(&self) -> SocketAddr {
        self.config.socket
    }

    /// Returns the number of times the RPC server will attempt to restart if it stops.
    pub const fn restart_count(&self) -> u32 {
        if self.config.no_restart { 0 } else { 3 }
    }

    /// Sets the given [`SocketAddr`] on the [`RpcBuilder`].
    pub const fn set_addr(mut self, addr: SocketAddr) -> Self {
        self.config.socket = addr;

        self
    }

    /// Registers the healthz endpoint on the [`RpcBuilder`].
    pub fn with_healthz(mut self) -> Result<Self, RegisterMethodError> {
        self.module.register_method("healthz", |_, _, _| {
            let response = HealthzResponse { version: std::env!("CARGO_PKG_VERSION").to_string() };
            jsonrpsee::core::RpcResult::Ok(response)
        })?;

        Ok(self)
    }

    /// Launches the jsonrpsee [`Server`].
    ///
    /// If the RPC server is disabled, this will return `Ok(None)`.
    ///
    /// ## Errors
    ///
    /// - [`RpcLauncherError::ServerStart`] if the server fails to start.
    pub async fn launch(self) -> Result<Option<ServerHandle>, RpcLauncherError> {
        if self.config.disabled {
            return Ok(None);
        }

        let server = Server::builder().build(self.config.socket).await?;
        Ok(Some(server.start(self.module)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_launch_no_modules() {
        let launcher = RpcBuilder::new(RpcConfig {
            disabled: false,
            socket: SocketAddr::from(([127, 0, 0, 1], 8080)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
        });
        let result = launcher.launch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_launch_with_modules() {
        let mut launcher = RpcBuilder::new(RpcConfig {
            disabled: false,
            socket: SocketAddr::from(([127, 0, 0, 1], 8081)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
        });
        launcher.merge(RpcModule::new(())).expect("module merge");
        launcher.merge::<()>(RpcModule::new(())).expect("module merge");
        launcher.merge(RpcModule::new(())).expect("module merge");
        let result = launcher.launch().await;
        assert!(result.is_ok());
    }
}
