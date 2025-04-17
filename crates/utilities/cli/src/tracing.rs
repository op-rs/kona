//! [tracing_subscriber] utilities.

use tracing::{Level, subscriber::SetGlobalDefaultError};
use tracing_subscriber::EnvFilter;

/// Initializes the tracing subscriber
///
/// # Arguments
/// * `verbosity_level` - The verbosity level (0-2)
/// * `env_filter` - Optional environment filter for the subscriber.
///
/// # Returns
/// * `Result<()>` - Ok if successful, Err otherwise.
pub fn init_tracing_subscriber(
    verbosity_level: u8,
    env_filter: Option<impl Into<EnvFilter>>,
) -> Result<(), SetGlobalDefaultError> {
    let level = match verbosity_level {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    let filter = env_filter.map(|e| e.into()).unwrap_or(EnvFilter::from_default_env());
    let filter = filter.add_directive(level.into());
    let subscriber = tracing_subscriber::fmt().with_max_level(level);
    tracing::subscriber::set_global_default(subscriber.with_env_filter(filter).finish())
}
