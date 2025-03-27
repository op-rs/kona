//! Contains RPC types specific to Kona's network stack.

mod request;
pub use request::NetRpcRequest;

mod server;
pub use server::NetworkRpc;
