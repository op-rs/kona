use crate::syncnode::ManagedNodeConfig;
use kona_interop::DependencySet;
use std::{net::SocketAddr, path::PathBuf};

use super::RollupConfigSet;

/// Configuration for the Supervisor service.
#[derive(Debug, Clone)]
pub struct Config {
    /// The URL of the L1 RPC endpoint.
    pub l1_rpc: String,

    /// L2 consensus nodes configuration.
    pub l2_consensus_nodes_config: Vec<ManagedNodeConfig>,

    /// Directory where the database files are stored.
    pub datadir: PathBuf,

    /// The socket address for the RPC server to listen on.
    pub rpc_addr: SocketAddr,

    /// The loaded dependency set configuration.
    pub dependency_set: DependencySet,

    /// The rollup configuration set.
    pub rollup_config_set: RollupConfigSet,
}
