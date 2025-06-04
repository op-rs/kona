//! Core types shared across supervisor components.
//!
//! This crate defines the fundamental data structures used within the
//! Optimism supervisor.
mod log;
pub use log::Log;

mod message;
pub use message::ExecutingMessage;

mod receipt;
pub use receipt::Receipts;

mod types;
pub use types::{BlockReplacement, BlockSeal, L2BlockRef, ManagedEvent, OutputV0};
