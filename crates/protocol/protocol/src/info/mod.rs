//! Module containing L1 Attributes types (aka the L1 block info transaction).

mod variant;
pub use variant::L1BlockInfoTx;

mod isthmus;
pub use isthmus::L1BlockInfoIsthmus;

mod bedrock;
pub use bedrock::L1BlockInfoBedrock;

mod ecotone;
pub use ecotone::L1BlockInfoEcotone;

mod interop;
pub use interop::L1BlockInfoInterop;

mod deposit_context;
pub use deposit_context::closing_deposit_context_tx;

mod errors;
pub use errors::{BlockInfoError, DecodeError};
