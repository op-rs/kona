//! Contains the `Superchains` type.

use alloc::vec::Vec;

use crate::Superchain;

/// A list of Hydrated Superchain Configs.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Superchains {
    /// A list of superchain configs.
    pub superchains: Vec<Superchain>,
}
