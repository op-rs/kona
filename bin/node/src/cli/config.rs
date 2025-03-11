//! RPC Config for the CLI.

/// The RPC Config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RPCConfig {
    /// The address to listen on.
    pub listen_addr: String,
    /// The port to listen on.
    pub listen_port: String,
    /// Should admin be anabled.
    pub enable_admin: bool,
}

impl RPCConfig {
    /// Returns the http endpoint associated with given `RPCConfig`.
    pub fn http_endpoint(&self) -> String {
        format!("http://{}/{}", self.listen_addr, self.listen_port)
    }
}
