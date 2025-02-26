//! Parser functions for CLI arguments.

use alloy_primitives::{hex, Bytes, B256};
use std::str::FromStr;

/// Parse a string slice into [B256].
pub fn parse_b256(s: &str) -> Result<B256, String> {
    B256::from_str(s).map_err(|_| format!("Invalid B256 value: {}", s))
}

/// Parse a string slice into [Bytes].
pub fn parse_bytes(s: &str) -> Result<Bytes, String> {
    hex::decode(s).map_err(|e| format!("Invalid hex string: {}", e)).map(Bytes::from)
}
