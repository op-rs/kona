#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RPCConfig {
    pub listen_addr: String,
    pub listen_port: String,
    pub enable_admin: bool,
}

impl RPCConfig {
    pub fn http_endpoint(&self) -> String {
        format!("http://{}/{}", self.listen_addr, self.listen_port)
    }
}
