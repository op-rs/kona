//! Contains the RPC Configuration.

use crate::RpcLauncher;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

/// The RPC configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// If the RPC is enabled.
    pub enabled: bool,
    /// The RPC listening address.
    pub listen_addr: IpAddr,
    /// The RPC listening port.
    pub listen_port: u16,
    /// Enable the admin API.
    pub enable_admin: bool,
    /// File path used to persist state changes made via the admin API so they persist across
    /// restarts.
    pub admin_persistence: Option<PathBuf>,
}

impl RpcConfig {
    /// Converts the [`RpcConfig`] into a [`RpcLauncher`].
    pub fn as_launcher(&self) -> RpcLauncher {
        let mut launcher = RpcLauncher::from(SocketAddr::from(self));
        if !self.enabled {
            launcher.disable();
        }
        launcher
    }
}

impl From<&RpcConfig> for SocketAddr {
    fn from(config: &RpcConfig) -> Self {
        Self::new(config.listen_addr, config.listen_port)
    }
}

impl From<&RpcConfig> for RpcLauncher {
    fn from(config: &RpcConfig) -> Self {
        config.as_launcher()
    }
}
