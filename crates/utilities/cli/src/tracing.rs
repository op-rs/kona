//! [tracing_subscriber] utilities.

use tracing::{Level, subscriber::SetGlobalDefaultError};
use tracing_subscriber::EnvFilter;

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
) -> Result<(), SetGlobalDefaultError> {
    let level = match verbosity_level {
        1 => Level::ERROR,
        2 => Level::WARN,
        3 => Level::INFO,
        4 => Level::DEBUG,
        _ => Level::TRACE,
    };
    if verbosity_level == 0 {
        return tracing::subscriber::set_global_default(tracing_subscriber::fmt().finish());
    }
    let filter = env_filter.map(|e| e.into()).unwrap_or(EnvFilter::from_default_env());
    let filter = filter.add_directive(level.into());
    let subscriber = tracing_subscriber::fmt().with_max_level(level);
    tracing::subscriber::set_global_default(subscriber.with_env_filter(filter).finish())
}

/// This provides function for init tracing in testing
///
/// # Functions
/// - `init_test_tracing`: A helper function for initializing tracing in test environments.
/// - `init_tracing_subscriber`: Initializes the tracing subscriber with a specified verbosity level
///   and optional environment filter.
pub fn init_test_tracing() {
    let _ = init_tracing_subscriber(4, None::<EnvFilter>);
}
