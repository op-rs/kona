use tokio::sync::oneshot::Sender;

use crate::EngineState;

/// The type of data that can be requested from the engine.
pub type EngineQuerySender = tokio::sync::mpsc::Sender<EngineStateQuery>;

/// Returns the full engine state.
#[derive(Debug)]
pub struct EngineStateQuery(Sender<EngineState>);

impl EngineStateQuery {
    /// Handles the engine query request.
    pub fn handle(self, state_recv: &tokio::sync::watch::Receiver<EngineState>) -> Option<()> {
        let state = *state_recv.borrow();
        self.0.send(state).ok()
    }
}
