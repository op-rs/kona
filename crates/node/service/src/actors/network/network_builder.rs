//! Contains traits for working with Network Builder.

use core::fmt::Debug;

use super::{NetworkBuilderError, NetworkDriver};

/// Contains methods for building a [`NetworkDriver`].
pub trait NetworkBuilderExt: Debug + Send {
    /// Builds a [`NetworkDriver`].
    fn build(self) -> Result<NetworkDriver, NetworkBuilderError>;
}
