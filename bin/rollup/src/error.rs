//! Error types for the rollup binary.

use thiserror::Error;

/// Errors that can occur in the rollup binary.
#[derive(Error, Debug)]
pub enum RollupError {
    /// CLI error from kona-cli utilities.
    #[error(transparent)]
    Cli(#[from] kona_cli::CliError),

    /// Error when rollup functionality is not yet implemented.
    #[error("Rollup CLI is not yet implemented")]
    NotImplemented,
}

/// Type alias for rollup results.
pub type RollupResult<T> = Result<T, RollupError>;
