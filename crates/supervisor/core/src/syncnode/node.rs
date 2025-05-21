
use crate::syncnode::types::ManagedEvent;
use std::sync::Arc;
use alloy_primitives::U256;
use jsonrpsee::core::client::{Subscription, SubscriptionClientT};
use jsonrpsee::core::middleware::RpcServiceBuilder;
use jsonrpsee::ws_client::WsClientBuilder;
use jsonrpsee::rpc_params;

#[derive(Debug)]
pub struct ManagedNodeConfig {
    pub chain_id: U256,
    pub url: String,
    pub port: u16,
    pub jwt_secret: String,
}
#[derive(Debug)]
pub struct NodeSubscriber {
    config: Arc<ManagedNodeConfig>,
    events: ManagedEvent
}

impl NodeSubscriber {
    pub async fn start_subscription(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .expect("Failed to initialize tracing subscriber");

        let ws_url = format!("ws://{}:{}", self.config.url, self.config.port);
        let client = WsClientBuilder::default()
            .set_rpc_middleware(RpcServiceBuilder::new().rpc_logger(1024))
            .build(&ws_url)
            .await?;

        let mut subscription: Subscription<i32> = client
            .subscribe("interop", rpc_params!["events"], "Unsubscribe")
            .await?;

        let future  = subscription.next().await;
        Ok(())
    }
}


