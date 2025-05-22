//! Providers for supervisor state tracking.
//!
//! This module defines and implements storage providers used by the supervisor
//! for managing L2 execution state. It includes support for reading and writing:
//! - Logs and block metadata (via [LogProvider](log_provider::LogProvider))
//! - Derivation pipeline state
//! - Chain head tracking and progression
mod log_provider;
