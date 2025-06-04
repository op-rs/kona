//! Configuration management for the supervisor.

mod rollup_config_set;
pub use rollup_config_set::{Genesis, RollupConfig, RollupConfigSet};

mod config;
pub use config::Config;