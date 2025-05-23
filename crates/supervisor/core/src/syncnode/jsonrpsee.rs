//! [`ManagedNodeApi`] trait contains all endpoints for supervisor <> node
//! communication in managed node.

use crate::types::ManagedEvent;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};

/// Using the proc_macro to generate the client and server code.
/// Default namespace separator is `_`.
#[rpc(client, server, namespace = "interop")]
pub trait ManagedNodeApi {
    /// Subscribe to the events from the managed node.
    #[subscription(name = "events", item = Option<ManagedEvent>, unsubscribe = "unsubscribeEvents")]
    async fn subscribe_events(&self) -> SubscriptionResult;

    /// Pull an event from the managed node.
    #[method(name = "pullEvent")]
    async fn pull_event(&self) -> RpcResult<ManagedEvent>;
}
