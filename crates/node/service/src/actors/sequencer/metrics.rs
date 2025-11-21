use std::time::Duration;

use crate::{
    BlockBuildingClient, Conductor, OriginSelector, SequencerActor, UnsafePayloadGossipClient,
};
use kona_derive::AttributesBuilder;

/// SequencerActor metrics-related method implementations.
impl<AB, BB, C, G, OS> SequencerActor<AB, BB, C, G, OS>
where
    AB: AttributesBuilder,
    C: Conductor,
    G: UnsafePayloadGossipClient,
    OS: OriginSelector,
    BB: BlockBuildingClient,
{
    /// Updates the metrics for the sequencer actor.
    pub(super) fn update_metrics(&self) {
        // no-op if disabled.
        #[cfg(feature = "metrics")]
        {
            let state_flags: [(&str, String); 2] = [
                ("active", self.is_active.to_string()),
                ("recovery", self.in_recovery_mode.to_string()),
            ];

            let gauge = metrics::gauge!(crate::Metrics::SEQUENCER_STATE, &state_flags);
            gauge.set(1);
        }
    }
}

#[inline]
pub(super) fn update_attributes_build_duration_metrics(duration: Duration) {
    // Log the attributes build duration, if metrics are enabled.
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_ATTRIBUTES_BUILDER_DURATION, duration);
}

#[inline]
pub(super) fn update_conductor_commitment_duration_metrics(duration: Duration) {
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_CONDUCTOR_COMMITMENT_DURATION, duration);
}

#[inline]
pub(super) fn update_block_build_duration_metrics(duration: Duration) {
    kona_macros::set!(
        gauge,
        crate::Metrics::SEQUENCER_BLOCK_BUILDING_START_TASK_DURATION,
        duration
    );
}

#[inline]
pub(super) fn update_seal_duration_metrics(duration: Duration) {
    // Log the block building seal task duration, if metrics are enabled.
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_BLOCK_BUILDING_SEAL_TASK_DURATION, duration);
}
