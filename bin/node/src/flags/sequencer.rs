//! Sequencer CLI Flags
//!
//! These are based on sequencer flags from the [`op-node`][op-node] CLI.
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go#L233-L265

use clap::Parser;
use std::{net::SocketAddr, num::ParseIntError, time::Duration};

/// Sequencer CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct SequencerArgs {
    /// Enable sequencing of new L2 blocks. A separate batch submitter has to be deployed to
    /// publish the data for verifiers.
    #[arg(
        long = "sequencer.enabled",
        default_value = "false",
        env = "KONA_NODE_SEQUENCER_ENABLED"
    )]
    pub enabled: bool,

    /// Initialize the sequencer in a stopped state. The sequencer can be started using the
    /// admin_startSequencer RPC.
    #[arg(
        long = "sequencer.stopped",
        default_value = "false",
        env = "KONA_NODE_SEQUENCER_STOPPED"
    )]
    pub stopped: bool,

    /// Maximum number of L2 blocks for restricting the distance between L2 safe and unsafe.
    /// Disabled if 0.
    #[arg(
        long = "sequencer.max-safe-lag",
        default_value = "0",
        env = "KONA_NODE_SEQUENCER_MAX_SAFE_LAG"
    )]
    pub max_safe_lag: u64,

    /// Number of L1 blocks to keep distance from the L1 head as a sequencer for picking an L1
    /// origin.
    #[arg(long = "sequencer.l1-confs", default_value = "4", env = "KONA_NODE_SEQUENCER_L1_CONFS")]
    pub l1_confs: u64,

    /// Forces the sequencer to strictly prepare the next L1 origin and create empty L2 blocks
    #[arg(
        long = "sequencer.recover",
        default_value = "false",
        env = "KONA_NODE_SSEQUENCER_RECOVER"
    )]
    pub recover: bool,

    /// Enable the conductor service.
    #[arg(
        long = "conductor.enabled",
        env = "KONA_NODE_CONDUCTOR_ENABLED",
        default_value = "false"
    )]
    pub conductor_enabled: bool,

    /// Conductor service rpc endpoint.
    #[arg(
        long = "conductor.rpc",
        env = "KONA_NODE_CONDUCTOR_RPC",
        default_value = "127.0.0.1:8547"
    )]
    pub conductor_rpc: Option<SocketAddr>,

    /// Conductor service rpc timeout.
    #[arg(
        long = "conductor.rpc.timeout",
        default_value = "1",
        env = "KONA_NODE_CONDUCTOR_RPC_TIMEOUT",
        value_parser = |arg: &str| -> Result<Duration, ParseIntError> {Ok(Duration::from_secs(arg.parse()?))}
    )]
    pub conductor_rpc_timeout: Duration,
}

impl Default for SequencerArgs {
    fn default() -> Self {
        // Construct default values using the clap parser.
        // This works since none of the cli flags are required.
        Self::parse_from::<[_; 0], &str>([])
    }
}
