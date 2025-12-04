//! Module containing L1 Attributes types (aka the L1 block info transaction).

mod variant;
pub use variant::L1BlockInfoTx;

mod bedrock;
pub use bedrock::{L1BlockInfoBedrock, L1BlockInfoBedrockFields, L1BlockInfoBedrockOnlyFields};

mod bedrock_base;
pub use bedrock_base::L1BlockInfoBedrockBaseFields;

mod ecotone;
pub use ecotone::{L1BlockInfoEcotone, L1BlockInfoEcotoneFields, L1BlockInfoEcotoneOnlyFields};

mod ecotone_base;
pub use ecotone_base::L1BlockInfoEcotoneBaseFields;

mod isthmus;
pub use isthmus::{L1BlockInfoIsthmus, L1BlockInfoIsthmusBaseFields, L1BlockInfoIsthmusFields};

mod jovian;
pub use jovian::{L1BlockInfoJovian, L1BlockInfoJovianBaseFields, L1BlockInfoJovianFields};

mod errors;
pub use errors::{BlockInfoError, DecodeError};

mod common;
pub(crate) use common::CommonL1BlockFields;
