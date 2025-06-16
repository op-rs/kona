//! Core [`RollupNodeService`] trait.

use super::{SequencerNodeService, ValidatorNodeService};
use async_trait::async_trait;
use derive_more::Display;

/// The [`NodeMode`] enum represents the modes of operation for the [`RollupNodeService`].
#[derive(Debug, Display, Default, Clone, Copy, PartialEq, Eq)]
pub enum NodeMode {
    /// Validator mode.
    #[display("Validator")]
    #[default]
    Validator,
    /// Sequencer mode.
    #[display("Sequencer")]
    Sequencer,
}

/// The [`RollupNodeService`] trait defines the core functionality of a rollup node. It can operate
/// in two modes: as a [`ValidatorNodeService`] or as a [`SequencerNodeService`].
///
/// [`SequencerNodeService`]: super::SequencerNodeService
#[async_trait]
pub trait RollupNodeService: ValidatorNodeService + SequencerNodeService {
    /// Returns the [NodeMode] of the service.
    fn mode(&self) -> NodeMode;

    /// Starts the rollup node service, using the entry point respective to the [`NodeMode`].
    ///
    /// - [`NodeMode::Validator`]: [`ValidatorNodeService::start`]
    /// - [`NodeMode::Sequencer`]: [`SequencerNodeService::start`]
    async fn start(&self) -> Result<(), Self::Error> {
        info!(
            target: "rollup_node",
            mode = %self.mode(),
            chain_id = self.config().l2_chain_id,
            "Starting rollup node services"
        );
        for hf in self.config().hardforks.to_string().lines() {
            info!(target: "rollup_node", "{hf}");
        }

        match self.mode() {
            NodeMode::Validator => <Self as ValidatorNodeService>::start(self).await,
            NodeMode::Sequencer => <Self as SequencerNodeService>::start(self).await,
        }
    }
}
