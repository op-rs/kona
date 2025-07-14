//! Supervisor core syncnode module
//! This module provides the core functionality for managing nodes in the supervisor environment.

mod node;
pub use node::ManagedNode;

mod error;
pub use error::{
    AuthenticationError, ClientError, ManagedEventTaskError, ManagedNodeError, SubscriptionError,
};

mod traits;
pub use traits::{
    BlockProvider, ManagedNodeController, ManagedNodeDataProvider, ManagedNodeProvider,
    NodeSubscriber,
};

mod client;
pub use client::{Client, ClientConfig, ManagedNodeClient};

pub(super) mod metrics;
pub(super) mod resetter;
pub(super) mod task;
pub(super) mod utils;
