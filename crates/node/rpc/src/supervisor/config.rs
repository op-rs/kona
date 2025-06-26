//! Contains the Configuration for the supervisor RPC server.

use alloy_rpc_types_engine::JwtSecret;
use std::net::SocketAddr;

/// The RPC Config.
#[derive(Debug, Clone)]
pub struct SupervisorRpcConfig {
    /// If the RPC is disabled.
    /// By default, the RPC server is disabled.
    pub rpc_disabled: bool,
    /// The socket address for the RPC server.
    pub socket_address: SocketAddr,
    /// The JWT secret for the RPC server.
    pub jwt_secret: JwtSecret,
}

impl SupervisorRpcConfig {
    /// Returns if the rpc is disabled.
    pub const fn is_disabled(&self) -> bool {
        self.rpc_disabled
    }
}

// By default, the RPC server is disabled.
// As such, the socket address and JWT secret are unused
// and can be set to random values.
impl std::default::Default for SupervisorRpcConfig {
    fn default() -> Self {
        Self {
            rpc_disabled: true,
            socket_address: SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), 9333),
            jwt_secret: JwtSecret::random(),
        }
    }
}
