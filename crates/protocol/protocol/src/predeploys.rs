//! Addresses of OP pre-deploys.
//!
//! This module contains the addresses of various predeploy contracts in the OP Stack.
//! See the complete set of predeploys at <https://specs.optimism.io/protocol/predeploys.html#predeploys>

use alloy_primitives::{address, Address};

/// Container for all predeploy contract addresses
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Predeploys;

impl Predeploys {
    /// List of all predeploys.
    pub const ALL: [Address; 1] = [Self::L2_TO_L1_MESSAGE_PASSER];

    /// The L2 contract `L2ToL1MessagePasser`, stores commitments to withdrawal transactions.
    pub const L2_TO_L1_MESSAGE_PASSER: Address =
        address!("0x4200000000000000000000000000000000000016");
}
