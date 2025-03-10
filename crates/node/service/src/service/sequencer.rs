//! Sequencer node service.

use super::ValidatorNodeService;
use async_trait::async_trait;

/// TODO
#[async_trait]
pub trait SequencerNodeService: ValidatorNodeService {
    /// TODO
    async fn start(&self) -> Result<(), Self::Error> {
        unimplemented!("TODO")
    }
}
