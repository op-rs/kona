//! Example for creating a static test fixture for `kona-executor` from a live chain
//!
//! ## Usage
//!
//! ```sh
//! cargo run --release -p execution-fixture
//! ```
//!
//! ## Inputs
//!
//! The test fixture creator takes the following inputs:
//!
//! - `-v` or `--verbosity`: Verbosity level (0-2)
//! - `-r` or `--l2-rpc`: The L2 execution layer RPC URL to use. Must be archival.
//! - `-b` or `--block-number`: L2 block number to execute for the fixture.
//! - `-o` or `--output-dir`: (Optional) The output directory for the fixture. If not provided,
//!   defaults to `kona-executor`'s `testdata` directory.

use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use kona_cli::{LogFormat, init_tracing_subscriber};
use kona_executor::test_utils::ExecutorTestFixtureCreator;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;
use url::Url;

/// The execution fixture creation command.
#[derive(Parser, Debug, Clone)]
#[command(about = "Creates a static test fixture for `kona-executor` from a live chain")]
pub struct ExecutionFixtureCommand {
    /// Verbosity level (0-5).
    /// If set to 0, no logs are printed.
    /// By default, the verbosity level is set to 3 (info level).
    #[arg(long, short, default_value = "3", action = ArgAction::Count)]
    pub v: u8,
    /// The format of the logs. One of: full, json, pretty, compact.
    #[arg(long = "logs.format", short = 'f', default_value = "full")]
    pub logs_format: LogFormat,
    /// The L2 archive EL to use.
    #[arg(long, short = 'r')]
    pub l2_rpc: Url,
    /// L2 block number to execute.
    #[arg(long, short = 'b')]
    pub block_number: u64,
    /// The output directory for the fixture.
    #[arg(long, short = 'o')]
    pub output_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = ExecutionFixtureCommand::parse();
    init_tracing_subscriber(cli.v, None::<EnvFilter>, cli.logs_format)?;

    let output_dir = if let Some(output_dir) = cli.output_dir {
        output_dir
    } else {
        // Default to `crates/proof/executor/testdata`
        let output = std::process::Command::new(env!("CARGO"))
            .arg("locate-project")
            .arg("--workspace")
            .arg("--message-format=plain")
            .output()?
            .stdout;
        let workspace_root: PathBuf = String::from_utf8(output)?.trim().into();

        workspace_root
            .parent()
            .ok_or(anyhow!("Failed to locate workspace root"))?
            .join("crates/proof/executor/testdata")
    };

    ExecutorTestFixtureCreator::new(cli.l2_rpc.as_str(), cli.block_number, output_dir)
        .create_static_fixture()
        .await;

    info!(block_number = cli.block_number, "Successfully created static test fixture");
    Ok(())
}
