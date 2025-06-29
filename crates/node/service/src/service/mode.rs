//! Contains enums that configure the mode for the node to operate in.

/// The [`NodeMode`] enum represents the modes of operation for the [`RollupNodeService`].
///
/// [`RollupNodeService`]: crate::RollupNodeService
#[derive(
    Debug, derive_more::Display, derive_more::FromStr, Default, Clone, Copy, PartialEq, Eq,
)]
pub enum NodeMode {
    /// Validator mode.
    #[display("Validator")]
    #[default]
    Validator,
    /// Sequencer mode.
    #[display("Sequencer")]
    Sequencer,
}

/// The [`InteropMode`] enum represents how the node works with interop.
#[derive(Debug, derive_more::Display, Default, Clone, Copy, PartialEq, Eq)]
pub enum InteropMode {
    /// The node is in polled mode which means it is not managed by the supervisor.
    #[display("Polled")]
    #[default]
    Polled,
    /// The node is in indexed mode which means it is managed by the supervisor.
    #[display("Indexed")]
    Indexed,
}
