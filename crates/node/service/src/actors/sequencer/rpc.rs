//! The RPC server for the sequencer actor.
//! Mostly handles queries from the admin rpc.

use kona_derive::AttributesBuilder;
use kona_rpc::SequencerAdminQuery;

use crate::actors::sequencer::actor::SequencerActorState;

/// Error type for sequencer RPC operations
#[derive(Debug, thiserror::Error)]
pub(super) enum SequencerRpcError {
    /// An error occurred while sending a response to the admin query.
    #[error(
        "Failed to send response to admin query. The response channel was closed, this may mean that the rpc actor was shut down."
    )]
    SendResponse,
}

impl<AB: AttributesBuilder> SequencerActorState<AB> {
    pub(super) async fn handle_admin_query(
        &mut self,
        query: SequencerAdminQuery,
    ) -> Result<(), SequencerRpcError> {
        match query {
            SequencerAdminQuery::SequencerActive(tx) => {
                tx.send(self.is_active).map_err(|_| SequencerRpcError::SendResponse)?;
            }
            SequencerAdminQuery::StartSequencer => {
                info!(target: "sequencer", "Starting sequencer");
                self.is_active = true;
            }
            SequencerAdminQuery::StopSequencer => {
                info!(target: "sequencer", "Stopping sequencer");
                self.is_active = false;
            }
            SequencerAdminQuery::ConductorEnabled(tx) => {
                tx.send(self.conductor.is_some()).map_err(|_| SequencerRpcError::SendResponse)?;
            }
            SequencerAdminQuery::SetRecoveryMode(is_active) => {
                self.is_recovery_mode = is_active;
                info!(target: "sequencer", is_active, "Updated recovery mode");
            }
            SequencerAdminQuery::OverrideLeader => {
                if let Some(conductor) = self.conductor.as_mut() {
                    if let Err(e) = conductor.override_leader().await {
                        error!(target: "sequencer::rpc", "Failed to override leader: {}", e);
                    }
                    info!(target: "sequencer", "Overrode leader via the conductor service");
                }
            }
        }

        Ok(())
    }
}
