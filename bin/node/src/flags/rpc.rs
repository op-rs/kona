//! Rpc CLI Arguments
//!
//! Flags for configuring the RPC server.

use clap::Parser;
use kona_rpc::RpcConfig;
use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

/// RPC CLI Arguments
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
pub struct RpcArgs {
    /// Whether to enable the rpc server.
    #[arg(long = "rpc.disabled", default_value = "false", env = "KONA_NODE_RPC_ENABLED")]
    pub rpc_disabled: bool,
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

impl Default for RpcArgs {
    fn default() -> Self {
        Self {
            rpc_disabled: false,
            listen_addr: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            listen_port: 9545,
            enable_admin: false,
            admin_persistence: None,
        }
    }
}

impl From<&RpcArgs> for RpcConfig {
    fn from(args: &RpcArgs) -> Self {
        Self {
            enabled: !args.rpc_disabled,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::disable_rpc(&["--rpc.disabled"], |args: &mut RpcArgs| { args.rpc_disabled = true; })]
    #[case::disable_rpc(&["--rpc.addr", "1.1.1.1"], |args: &mut RpcArgs| { args.listen_addr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)); })]
    #[case::disable_rpc(&["--rpc.port", "8743"], |args: &mut RpcArgs| { args.listen_port = 8743; })]
    #[case::disable_rpc(&["--rpc.enable-admin"], |args: &mut RpcArgs| { args.enable_admin = true; })]
    #[case::disable_rpc(&["--rpc.admin-state", "/"], |args: &mut RpcArgs| { args.admin_persistence = Some(PathBuf::from("/")); })]
    fn test_parse_rpc_args(#[case] args: &[&str], #[case] mutate: impl Fn(&mut RpcArgs)) {
        let args = [&["kona-node"], args].concat();
        let cli = RpcArgs::parse_from(args);
        let mut expected = RpcArgs::default();
        mutate(&mut expected);
        assert_eq!(cli, expected);
    }
}
