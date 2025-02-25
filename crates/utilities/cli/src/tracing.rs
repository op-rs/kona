//! [tracing_subscriber] utilities.

use tracing::{subscriber::SetGlobalDefaultError, Level};

/// Initializes the tracing subscriber
///
/// # Arguments
/// * `verbosity_level` - The verbosity level (0-2)
///
/// # Returns
/// * `Result<()>` - Ok if successful, Err otherwise.
pub fn init_tracing_subscriber(verbosity_level: u8) -> Result<(), SetGlobalDefaultError> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(match verbosity_level {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)
}
