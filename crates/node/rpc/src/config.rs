//! Contains the RPC Configuration.

use std::{net::SocketAddr, path::PathBuf};

/// A trait for the [`RpcBuilder`]
pub trait RpcBuilderProvider {
    /// Returns whether WebSocket RPC endpoint is enabled
    fn ws_enabled(&self) -> bool;

    /// Returns whether development RPC endpoints are enabled
    fn dev_enabled(&self) -> bool;

    /// Returns the socket address of the [`RpcBuilder`].
    fn socket(&self) -> SocketAddr;

    /// Returns the number of times the RPC server will attempt to restart if it stops.
    fn restart_count(&self) -> u32;

    /// Sets the given [`SocketAddr`] on the [`RpcBuilder`].
    fn set_addr(&mut self, addr: SocketAddr);
}

/// The RPC configuration.
#[derive(Debug, Clone)]
pub struct RpcBuilder {
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
    /// Enable development RPC endpoints
    pub dev_enabled: bool,
}

impl RpcBuilderProvider for RpcBuilder {
    fn ws_enabled(&self) -> bool {
        self.ws_enabled
    }

    fn dev_enabled(&self) -> bool {
        self.dev_enabled
    }

    fn socket(&self) -> SocketAddr {
        self.socket
    }

    fn restart_count(&self) -> u32 {
        if self.no_restart { 0 } else { 3 }
    }

    fn set_addr(&mut self, addr: SocketAddr) {
        self.socket = addr;
    }
}
