//! The [`NodeMode`] enum.

/// The [`NodeMode`] enum represents the modes of operation for the [`RollupNodeService`].
///
/// [`RollupNodeService`]: crate::RollupNodeService
#[derive(Debug, derive_more::Display, Default, Clone, Copy, PartialEq, Eq)]
pub enum NodeMode {
    /// Validator mode.
    #[display("Validator")]
    #[default]
    Validator,
    /// Sequencer mode.
    #[display("Sequencer")]
    Sequencer,
}
