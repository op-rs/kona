use std::net::SocketAddr;
use jsonrpsee::{core::RegisterMethodError, server::Server, RpcModule};
use tracing::info;

#[derive(Clone)]
pub struct RpcServer {
    endpoint: SocketAddr,
}

impl RpcServer {
    pub fn new(listen_addr: &str, listen_port: u16) -> Self {
        let endpoint = format!("{}:{}", listen_addr, listen_port).parse().unwrap();
        Self { endpoint }
    }

    pub async fn start(&self) -> Result<(), RegisterMethodError> {
        let server = Server::builder().build(self.endpoint).await.unwrap();
        let mut module = RpcModule::new(());

        module.register_method("optimism_echo", |params,_, _| {
            let input: String = params.one().unwrap();
            format!("Echo: {}", input)
        })?;

        let handle = server.start(module);
        info!("RPC Server started at {}", self.endpoint);
        handle.stopped().await;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::http_client::HttpClientBuilder;
    use jsonrpsee_core::client::ClientT;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_optimism_echo() {
        let rpc_server = RpcServer::new("127.0.0.1", 8545);
        tokio::spawn(async move {
            let _ = rpc_server.start().await;
        });

        sleep(Duration::from_secs(1)).await;

        let my_client = HttpClientBuilder::default()
            .build("http://127.0.0.1:8545").unwrap();

        let response: String = my_client.request("optimism_echo", ["OP"].as_ref()).await.unwrap();
        assert_eq!(response, "Echo: OP");
    }
}