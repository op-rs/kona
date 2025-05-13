//! Log indexing module for processing L2 receipts and extracting messages.
//!
//! This module provides functionality to extract and persist [`ExecutingMessage`]s and their
//! corresponding [`LogEntry`]s from L2 block receipts. It handles computing message payload hashes
//! and log hashes based on the interop messaging specification.
//!
//! # Modules
//!
//! - [`LogIndexer`] — main indexer that processes logs and persists them via [`StateManager`].
//! - [`LogIndexerError`] — error type for failures in fetching or storing logs.
//! - `util` — helper functions for computing payload and log hashes.
//! - `types` — definitions for parsed messages and structured log entries.

mod error;
pub use error::LogIndexerError;

mod indexer;
pub use indexer::LogIndexer;
mod util;
pub use util::{payload_hash_to_log_hash, log_to_log_hash, log_to_message_payload};

mod types;
pub use types::{ExecutingMessage, LogEntry};

