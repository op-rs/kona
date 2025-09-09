//! [tracing_subscriber] utilities.

use tracing_subscriber::{
    Layer,
    prelude::__tracing_subscriber_SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
};

use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use crate::{LogConfig, LogRotation};

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

impl LogConfig {
    /// Initializes the tracing subscriber
    ///
    /// # Arguments
    /// * `verbosity_level` - The verbosity level (0-5). If `0`, no logs are printed.
    /// * `env_filter` - Optional environment filter for the subscriber.
    ///
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise.
    pub fn init_tracing_subscriber(
        &self,
        env_filter: Option<EnvFilter>,
    ) -> Result<(), TryInitError> {
        let file_layer = self.file_logs.as_ref().map(|file_logs| {
            let directory_path = file_logs.directory_path.clone();

            let appender = match file_logs.rotation {
                LogRotation::Minutely => {
                    tracing_appender::rolling::minutely(directory_path, "kona.log")
                }
                LogRotation::Hourly => {
                    tracing_appender::rolling::hourly(directory_path, "kona.log")
                }
                LogRotation::Daily => tracing_appender::rolling::daily(directory_path, "kona.log"),
                LogRotation::Never => tracing_appender::rolling::never(directory_path, "kona.log"),
            };

            match file_logs.format {
                LogFormat::Full => tracing_subscriber::fmt::layer().with_writer(appender).boxed(),
                LogFormat::Json => {
                    tracing_subscriber::fmt::layer().json().with_writer(appender).boxed()
                }
                LogFormat::Pretty => {
                    tracing_subscriber::fmt::layer().pretty().with_writer(appender).boxed()
                }
                LogFormat::Compact => {
                    tracing_subscriber::fmt::layer().compact().with_writer(appender).boxed()
                }
            }
        });

        let stdout_layer = self.stdout_logs.as_ref().map(|stdout_logs| match stdout_logs.format {
            LogFormat::Full => tracing_subscriber::fmt::layer().boxed(),
            LogFormat::Json => tracing_subscriber::fmt::layer().json().boxed(),
            LogFormat::Pretty => tracing_subscriber::fmt::layer().pretty().boxed(),
            LogFormat::Compact => tracing_subscriber::fmt::layer().compact().boxed(),
        });

        let env_filter = env_filter
            .unwrap_or(EnvFilter::from_default_env())
            .add_directive(self.global_level.into());

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stdout_layer)
            .try_init()?;

        Ok(())
    }
}

/// This provides function for init tracing in testing
///
/// # Functions
/// - `init_test_tracing`: A helper function for initializing tracing in test environments.
/// - `init_tracing_subscriber`: Initializes the tracing subscriber with a specified verbosity level
///   and optional environment filter.
pub fn init_test_tracing() {
    let _ = LogConfig::default().init_tracing_subscriber(None::<EnvFilter>);
}
