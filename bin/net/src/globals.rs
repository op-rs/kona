//! Global arguments for the CLI.

use clap::Parser;

/// Global arguments for the CLI.
#[derive(Parser, Clone, Debug)]
pub(crate) struct GlobalArgs {
    /// The L2 chain ID to use.
    #[clap(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// A port to serve prometheus metrics on.
    #[clap(
        long,
        short = 'm',
        default_value = "9090",
        help = "The port to serve prometheus metrics on"
    )]
    pub metrics_port: u16,
}
