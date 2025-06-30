//! Supervisor core syncnode module
//! This module provides the core functionality for managing nodes in the supervisor environment.

mod node;
pub use node::{ManagedNode, ManagedNodeConfig};

mod error;
pub use error::{AuthenticationError, ManagedEventTaskError, ManagedNodeError, SubscriptionError};

mod task;
mod traits;
pub use traits::{ManagedNodeApiProvider, ManagedNodeProvider, NodeSubscriber, ReceiptProvider};

pub use task::ManagedEventTask;

pub(crate) mod metrics;
