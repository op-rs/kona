//! Contains the rollup CLI.

use crate::{error::RollupResult, version};
use clap::Parser;
use kona_cli::{GlobalArgs, cli_styles};

/// The rollup CLI.
#[derive(Parser, Clone, Debug)]
#[command(
    author,
    version = version::SHORT_VERSION,
    long_version = version::LONG_VERSION,
    about,
    styles = cli_styles(),
    long_about = None
)]
pub struct Cli {
    /// Global arguments for the CLI.
    #[command(flatten)]
    pub global: GlobalArgs,
}

impl Cli {
    /// Runs the rollup binary.
    pub fn run(self) -> RollupResult<()> {
        Err(crate::error::RollupError::NotImplemented)
    }
}
