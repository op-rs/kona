//! Contains the RPC Configuration.

use jsonrpsee::RpcModule;

use crate::RpcLauncher;
use std::{net::SocketAddr, path::PathBuf};

/// The RPC configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// Disable the rpc server.
    pub disabled: bool,
    /// Prevent the rpc server from being restarted.
    pub no_restart: bool,
    /// The RPC socket address.
    pub socket: SocketAddr,
    /// Enable the admin API.
    pub enable_admin: bool,
    /// File path used to persist state changes made via the admin API so they persist across
    /// restarts.
    pub admin_persistence: Option<PathBuf>,
    /// Enable the websocket rpc server
    pub ws_enabled: bool,
}

impl RpcConfig {
    /// Converts the [`RpcConfig`] into a [`RpcLauncher`].
    pub fn as_launcher(self) -> RpcLauncher {
        RpcLauncher { config: self, module: RpcModule::new(()) }
    }
}

impl From<RpcConfig> for RpcLauncher {
    fn from(config: RpcConfig) -> Self {
        config.as_launcher()
    }
}
