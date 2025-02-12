//! Constants for the OP Stack interop protocol.

use alloy_primitives::{address, Address};

/// The address of the L2 cross chain inbox predeploy proxy.
pub const CROSS_L2_INBOX_ADDRESS: Address = address!("4200000000000000000000000000000000000022");

/// The expiry window for relaying an initiating message (in seconds).
/// <https://specs.optimism.io/interop/messaging.html#message-expiry-invariant>
pub const MESSAGE_EXPIRY_WINDOW: u64 = 180 * 24 * 60 * 60;

/// The current version of the [SuperRoot] encoding format.
///
/// [SuperRoot]: crate::SuperRoot
pub const SUPER_ROOT_VERSION: u8 = 1;
