//! Rpc CLI Arguments
//!
//! Flags for configuring the RPC server.

use clap::Parser;
use kona_rpc::RpcConfig;
use std::{net::IpAddr, path::PathBuf};

/// RPC CLI Arguments
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
pub struct RpcArgs {
    /// RPC listening address.
    #[arg(long = "rpc.addr", default_value = "0.0.0.0", env = "KONA_NODE_RPC_ADDR")]
    pub listen_addr: IpAddr,
    /// RPC listening port.
    #[arg(long = "rpc.port", default_value = "9545", env = "KONA_NODE_RPC_PORT")]
    pub listen_port: u16,
    /// Enable the admin API.
    #[arg(long = "rpc.enable-admin", env = "KONA_NODE_RPC_ENABLE_ADMIN")]
    pub enable_admin: bool,
    /// File path used to persist state changes made via the admin API so they persist across
    /// restarts. Disabled if not set.
    #[arg(long = "rpc.admin-state", env = "KONA_NODE_RPC_ADMIN_STATE")]
    pub admin_persistence: Option<PathBuf>,
}

impl From<&RpcArgs> for RpcConfig {
    fn from(args: &RpcArgs) -> Self {
        Self {
            listen_addr: args.listen_addr,
            listen_port: args.listen_port,
            enable_admin: args.enable_admin,
            admin_persistence: args.admin_persistence.clone(),
        }
    }
}

impl From<RpcArgs> for RpcConfig {
    fn from(args: RpcArgs) -> Self {
        Self::from(&args)
    }
}
