//! Arguments for logging.

use clap::{ArgAction, Args};
use tracing_subscriber::EnvFilter;

use crate::{init_tracing_subscriber, tracing::LogFormat};

/// Global configuration arguments.
#[derive(Args, Debug, Default, Clone)]
pub struct LogArgs {
    /// Verbosity level (0-5).
    /// If set to 0, no logs are printed.
    /// By default, the verbosity level is set to 3 (info level).
    #[arg(
        long,
        short,
        global = true,
        default_value = "3",
        env = "KONA_NODE_LOG_LEVEL",
        action = ArgAction::Count,
    )]
    pub v: u8,
    /// The format of the logs. One of: full, json, pretty, compact.
    #[arg(
        long = "logs.format",
        short = 'f',
        global = true,
        default_value = "full",
        env = "KONA_NODE_LOG_FORMAT"
    )]
    pub logs_format: LogFormat,
}

impl LogArgs {
    /// Initializes the telemetry stack.
    pub fn init_tracing(&self, filter: Option<EnvFilter>) -> anyhow::Result<()> {
        Ok(init_tracing_subscriber(self.v, filter, self.logs_format)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Helper struct to parse GlobalArgs within a test CLI structure
    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        global: LogArgs,
    }

    #[test]
    fn test_default_verbosity_level() {
        let cli = TestCli::parse_from(["test_app"]);
        assert_eq!(cli.global.v, 3, "Default verbosity should be 3 when no -v flag is present.");
    }

    #[test]
    fn test_verbosity_count() {
        let cli_v1 = TestCli::parse_from(["test_app", "-v"]);
        assert_eq!(cli_v1.global.v, 1, "Verbosity with a single -v should be 1.");

        let cli_v3 = TestCli::parse_from(["test_app", "-vvv"]);
        assert_eq!(cli_v3.global.v, 3, "Verbosity with -vvv should be 3.");

        let cli_v5 = TestCli::parse_from(["test_app", "-vvvvv"]);
        assert_eq!(cli_v5.global.v, 5, "Verbosity with -vvvvv should be 5.");
    }
}
