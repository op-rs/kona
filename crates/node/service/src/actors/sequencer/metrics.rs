use std::time::Duration;

use crate::{BlockBuildingClient, Conductor, OriginSelector, SequencerActor};
use kona_derive::AttributesBuilder;

/// SequencerActor metrics-related method implementations.
impl<AB, C, OS, BB> SequencerActor<AB, C, OS, BB>
where
    AB: AttributesBuilder,
    C: Conductor,
    OS: OriginSelector,
    BB: BlockBuildingClient,
{
    /// Updates the metrics for the sequencer actor.
    pub(in crate::actors::sequencer) fn update_metrics(&self) {
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

pub(in crate::actors::sequencer) fn update_build_duration_metrics(duration: Duration) {
    // Log the attributes build duration, if metrics are enabled.
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_ATTRIBUTES_BUILDER_DURATION, duration);
}

pub(in crate::actors::sequencer) fn update_seal_duration_metrics(duration: Duration) {
    // Log the block building seal task duration, if metrics are enabled.
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_BLOCK_BUILDING_SEAL_TASK_DURATION, duration);
}

pub(in crate::actors::sequencer) fn update_conductor_commitment_duration_metrics(
    duration: Duration,
) {
    kona_macros::set!(gauge, crate::Metrics::SEQUENCER_CONDUCTOR_COMMITMENT_DURATION, duration);
}
