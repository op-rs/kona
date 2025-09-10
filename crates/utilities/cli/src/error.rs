//! Error types for CLI utilities.

use thiserror::Error;

/// Errors that can occur in CLI operations.
#[derive(Error, Debug)]
pub enum CliError {
    /// Error when no chain config is found for the given chain ID.
    #[error("No chain config found for chain ID: {chain_id}")]
    ChainConfigNotFound {
        /// The chain ID that was not found.
        chain_id: u64,
    },

    /// Error when no roles are found for the given chain ID.
    #[error("No roles found for chain ID: {chain_id}")]
    RolesNotFound {
        /// The chain ID for which roles were not found.
        chain_id: u64,
    },

    /// Error when no unsafe block signer is found for the given chain ID.
    #[error("No unsafe block signer found for chain ID: {chain_id}")]
    UnsafeBlockSignerNotFound {
        /// The chain ID for which the signer was not found.
        chain_id: u64,
    },

    /// Error initializing metrics.
    #[error("Failed to initialize metrics: {source}")]
    MetricsInitialization {
        /// The underlying error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Type alias for CLI results.
pub type CliResult<T> = Result<T, CliError>;
