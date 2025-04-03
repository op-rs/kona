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
    let subscriber = tracing_subscriber::fmt().with_max_level(level);

    if let Some(env_filter) = env_filter {
        let env_filter = env_filter.into();
        let env_filter = env_filter.add_directive(level.into());
        tracing::subscriber::set_global_default(subscriber.with_env_filter(env_filter).finish())
    } else {
        tracing::subscriber::set_global_default(subscriber.finish())
    }
}
