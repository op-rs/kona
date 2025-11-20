use super::SequencerActor;
use crate::{BlockEngineClient, Conductor, OriginSelector, SequencerAdminQuery};
use alloy_primitives::B256;
use kona_derive::AttributesBuilder;
use kona_rpc::SequencerAdminAPIError;

/// Handler for the Sequencer Admin API.
impl<AB, C, OS, BE> SequencerActor<AB, C, OS, BE>
where
    AB: AttributesBuilder,
    C: Conductor,
    OS: OriginSelector,
    BE: BlockEngineClient,
{
    /// Handles the provided [`SequencerAdminQuery`], sending the response via the provided sender.
    /// This function is used to decouple admin API logic from the response mechanism (channels).
    pub(in crate::actors::sequencer) async fn handle_admin_query(
        &mut self,
        query: SequencerAdminQuery,
    ) {
        match query {
            SequencerAdminQuery::SequencerActive(tx) => {
                if tx.send(self.is_sequencer_active().await).is_err() {
                    warn!(target: "sequencer", "Failed to send response for is_sequencer_active query");
                }
            }
            SequencerAdminQuery::StartSequencer(tx) => {
                if tx.send(self.start_sequencer().await).is_err() {
                    warn!(target: "sequencer", "Failed to send response for start_sequencer query");
                }
            }
            SequencerAdminQuery::StopSequencer(tx) => {
                if tx.send(self.stop_sequencer().await).is_err() {
                    warn!(target: "sequencer", "Failed to send response for stop_sequencer query");
                }
            }
            SequencerAdminQuery::ConductorEnabled(tx) => {
                if tx.send(self.is_conductor_enabled().await).is_err() {
                    warn!(target: "sequencer", "Failed to send response for is_conductor_enabled query");
                }
            }
            SequencerAdminQuery::SetRecoveryMode(is_active, tx) => {
                if tx.send(self.set_recovery_mode(is_active).await).is_err() {
                    warn!(target: "sequencer", is_active = is_active, "Failed to send response for set_recovery_mode query");
                }
            }
            SequencerAdminQuery::OverrideLeader(tx) => {
                if tx.send(self.override_leader().await).is_err() {
                    warn!(target: "sequencer", "Failed to send response for override_leader query");
                }
            }
        }
    }

    pub(in crate::actors::sequencer) async fn is_sequencer_active(
        &self,
    ) -> Result<bool, SequencerAdminAPIError> {
        Ok(self.is_active)
    }

    pub(in crate::actors::sequencer) async fn is_conductor_enabled(
        &self,
    ) -> Result<bool, SequencerAdminAPIError> {
        Ok(self.conductor.is_some())
    }

    pub(in crate::actors::sequencer) async fn start_sequencer(
        &mut self,
    ) -> Result<(), SequencerAdminAPIError> {
        if self.is_active {
            info!(target: "sequencer", "received request to start sequencer, but it is already started");
            return Ok(());
        }

        info!(target: "sequencer", "Starting sequencer");
        self.is_active = true;

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }

    pub(in crate::actors::sequencer) async fn stop_sequencer(
        &mut self,
    ) -> Result<B256, SequencerAdminAPIError> {
        info!(target: "sequencer", "Stopping sequencer");
        self.is_active = false;

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(self.unsafe_head_rx.borrow().hash())
    }

    pub(in crate::actors::sequencer) async fn set_recovery_mode(
        &mut self,
        is_active: bool,
    ) -> Result<(), SequencerAdminAPIError> {
        self.in_recovery_mode = is_active;
        info!(target: "sequencer", is_active, "Updated recovery mode");

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }

    pub(in crate::actors::sequencer) async fn override_leader(
        &mut self,
    ) -> Result<(), SequencerAdminAPIError> {
        if let Some(conductor) = self.conductor.as_mut() {
            if let Err(e) = conductor.override_leader().await {
                error!(target: "sequencer::rpc", "Failed to override leader: {}", e);
                return Err(SequencerAdminAPIError::LeaderOverrideError(e.to_string()));
            }
            info!(target: "sequencer", "Overrode leader via the conductor service");
        }

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }
}
