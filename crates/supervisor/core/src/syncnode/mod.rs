//! Supervisor core syncnode module
//! This module provides the core functionality for managing nodes in the supervisor environment.

mod node;
pub use node::{ManagedNode, ManagedNodeConfig};

mod event;
pub use event::NodeEvent;

mod error;
pub use error::{AuthenticationError, ManagedNodeError, SubscriptionError};
