use clap::Args;
use kona_cli::init_prometheus_server;
use std::net::IpAddr;

/// Configuration for Prometheus metrics.
#[derive(Args, Debug)]
pub struct MetricsArgs {
    /// Controls whether prometheus metrics are enabled.
    /// Disabled by default.
    #[arg(
        long = "metrics.enabled",
        global = true,
        default_value_t = false,
        env = "METRICS_ENABLED"
    )]
    pub enabled: bool,
    /// The port to serve prometheus metrics on
    #[arg(long = "metrics.port", global = true, default_value = "9090", env = "METRICS_PORT")]
    pub port: u16,
    /// The ip address to use to emit prometheus metrics.
    #[arg(long = "metrics.addr", global = true, default_value = "0.0.0.0", env = "METRICS_ADDR")]
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
    use std::net::{IpAddr, Ipv4Addr};

    // Helper struct to parse MetricsArgs within a test CLI structure
    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        metrics: MetricsArgs,
    }

    #[test]
    fn test_default_metrics_args() {
        let cli = TestCli::parse_from(["test_app"]);
        assert!(!cli.metrics.enabled, "Default for metrics.enabled should be false.");
        assert_eq!(cli.metrics.port, 9090, "Default for metrics.port should be 9090.");
        assert_eq!(
            cli.metrics.addr,
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            "Default for metrics.addr should be 0.0.0.0."
        );
    }

    #[test]
    fn test_metrics_args_from_cli() {
        let cli = TestCli::parse_from([
            "test_app",
            "--metrics.enabled", // Presence of the flag sets it to true
            "--metrics.port",
            "9999",
            "--metrics.addr",
            "127.0.0.1",
        ]);
        assert!(
            cli.metrics.enabled,
            "metrics.enabled should be true when --metrics.enabled flag is passed."
        );
        assert_eq!(cli.metrics.port, 9999, "metrics.port should be parsed from CLI.");
        assert_eq!(
            cli.metrics.addr,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            "metrics.addr should be parsed from CLI."
        );
    }

    #[test]
    fn test_init_metrics_when_disabled() {
        let args =
            MetricsArgs { enabled: false, port: 1234, addr: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)) };
        let result = args.init_metrics();
        assert!(
            result.is_ok(),
            "init_metrics should return Ok when metrics are disabled. Error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_init_metrics_when_enabled() {
        let args = MetricsArgs {
            enabled: true,
            port: 9876,
            addr: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        };
        let result = args.init_metrics();
        assert!(
            result.is_ok(),
            "init_metrics should return Ok when metrics are enabled. Error: {:?}",
            result.err()
        );
    }
}
