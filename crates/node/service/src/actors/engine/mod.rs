//! The [`EngineActor`] and its components.

mod actor;
pub(crate) use actor::{BuildRequest, SealRequest};
pub use actor::{EngineActor, EngineConfig, EngineContext, EngineInboundData};

mod error;
pub use error::EngineError;

mod finalizer;

pub use finalizer::L2Finalizer;

mod rollup_boost;
