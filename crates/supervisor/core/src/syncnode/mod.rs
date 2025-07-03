//! Supervisor core syncnode module
//! This module provides the core functionality for managing nodes in the supervisor environment.

mod node;
pub use node::ManagedNode;

mod error;
pub use error::{AuthenticationError, ManagedEventTaskError, ManagedNodeError, SubscriptionError};

mod traits;
pub use traits::{ManagedNodeApiProvider, ManagedNodeProvider, NodeSubscriber, ReceiptProvider};

mod client;
pub use client::{Client, ClientConfig, ManagedNodeClient};

pub(super) mod metrics;
pub(super) mod resetter;
pub(super) mod task;
