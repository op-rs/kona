//! Prometheus metrics CLI args
//!
//! Specifies the available flags for prometheus metric configuration inside CLI

use std::net::IpAddr;

use clap::{Args, arg};
use kona_cli::init_prometheus_server;

/// The metric configuration available in CLI
#[derive(Debug, Clone, Args)]
pub struct MetricsArgs {
    /// Controls whether prometheus metrics are enabled.
    /// Disabled by default.
    #[arg(
        long = "metrics.enabled",
        global = true,
        default_value_t = false,
        env = "KONA_NODE_METRICS_ENABLED"
    )]
    pub enabled: bool,
    /// The port to serve prometheus metrics on
    #[arg(
        long = "metrics.port",
        global = true,
        default_value = "9090",
        env = "KONA_NODE_METRICS_PORT"
    )]
    pub port: u16,
    /// The ip address to use to emit prometheus metrics.
    #[arg(
        long = "metrics.addr",
        global = true,
        default_value = "0.0.0.0",
        env = "KONA_NODE_METRICS_ADDR"
    )]
    pub addr: IpAddr,
}

impl MetricsArgs {
    /// Initialize the tracing stack and Prometheus metrics recorder.
    ///
    /// This function should be called at the beginning of the program.
    pub fn init_metrics(&self) -> anyhow::Result<()> {
        if self.enabled {
            init_prometheus_server(self.addr, self.port)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// A mock command that uses the MetricsArgs.
    #[derive(Parser, Debug, Clone)]
    #[command(about = "Mock command")]
    struct MockCommand {
        /// Metrics CLI Flags
        #[clap(flatten)]
        pub metrics: MetricsArgs,
    }

    #[test]
    fn test_metrics_args_listen_enabled() {
        let args = MockCommand::parse_from(["test", "--metrics.enabled"]);
        assert!(args.metrics.enabled);

        let args = MockCommand::parse_from(["test"]);
        assert!(!args.metrics.enabled);
    }

    #[test]
    fn test_metrics_args_listen_ip() {
        let args = MockCommand::parse_from(["test", "--metrics.addr", "127.0.0.1"]);
        let expected: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(args.metrics.addr, expected);
    }

    #[test]
    fn test_metrics_args_listen_port() {
        let args = MockCommand::parse_from(["test", "--metrics.port", "1234"]);
        assert_eq!(args.metrics.port, 1234);
    }
}
