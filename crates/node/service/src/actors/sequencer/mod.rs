//! The `SequencerActor` and its components.

mod origin_selector;
pub use origin_selector::{L1OriginSelector, L1OriginSelectorError};

mod actor;
pub use actor::{
    SequencerActor, SequencerActorError, SequencerActorState, SequencerContext,
    SequencerOutboundData,
};
