//! [tracing_subscriber] utilities.

use serde::{Deserialize, Serialize};
use tracing::{level_filters::LevelFilter, subscriber::SetGlobalDefaultError};
use tracing_subscriber::EnvFilter;

/// The format of the logs.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum LogFormat {
    /// Full format (default).
    #[default]
    Full,
    /// JSON format.
    Json,
    /// Pretty format.
    Pretty,
    /// Compact format.
    Compact,
}

/// Initializes the tracing subscriber
///
/// # Arguments
/// * `verbosity_level` - The verbosity level (0-5). If `0`, no logs are printed.
/// * `env_filter` - Optional environment filter for the subscriber.
///
/// # Returns
/// * `Result<()>` - Ok if successful, Err otherwise.
pub fn init_tracing_subscriber(
    verbosity_level: u8,
    env_filter: Option<impl Into<EnvFilter>>,
    log_format: LogFormat,
) -> Result<(), SetGlobalDefaultError> {
    let level = match verbosity_level {
        0 => LevelFilter::OFF,
        1 => LevelFilter::ERROR,
        2 => LevelFilter::WARN,
        3 => LevelFilter::INFO,
        4 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    };
    let filter = env_filter.map(|e| e.into()).unwrap_or(EnvFilter::from_default_env());
    let filter = filter.add_directive(level.into());

    match log_format {
        LogFormat::Full => {
            let subscriber =
                tracing_subscriber::fmt().with_max_level(level).with_env_filter(filter);
            tracing::subscriber::set_global_default(subscriber.finish())?;
        }
        LogFormat::Json => {
            let subscriber =
                tracing_subscriber::fmt().json().with_max_level(level).with_env_filter(filter);
            tracing::subscriber::set_global_default(subscriber.finish())?;
        }
        LogFormat::Pretty => {
            let subscriber =
                tracing_subscriber::fmt().pretty().with_max_level(level).with_env_filter(filter);
            tracing::subscriber::set_global_default(subscriber.finish())?;
        }
        LogFormat::Compact => {
            let subscriber =
                tracing_subscriber::fmt().compact().with_max_level(level).with_env_filter(filter);
            tracing::subscriber::set_global_default(subscriber.finish())?;
        }
    };

    Ok(())
}

/// This provides function for init tracing in testing
///
/// # Functions
/// - `init_test_tracing`: A helper function for initializing tracing in test environments.
/// - `init_tracing_subscriber`: Initializes the tracing subscriber with a specified verbosity level
///   and optional environment filter.
pub fn init_test_tracing() {
    let _ = init_tracing_subscriber(4, None::<EnvFilter>, LogFormat::Full);
}
