//! Integration tests for the network actor.

use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};

use crate::actors::{
    network::mocks::{TestNetwork, TestNetworkError},
    utils::SEED_GENERATOR_BUILDER,
};

pub(super) mod mocks;

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_network_conn() -> anyhow::Result<()> {
    let mut seed_generator = SEED_GENERATOR_BUILDER.next_generator();

    let network_1 = TestNetwork::new(vec![], &mut seed_generator);
    let enr_1 = network_1.peer_enr().await?;

    let network_2 = TestNetwork::new(vec![enr_1], &mut seed_generator);

    (async || network_2.is_connected_to(&network_1).await)
        .retry(ExponentialBuilder::default().with_total_delay(Some(Duration::from_secs(10))))
        // When to retry
        .when(|e| matches!(e, TestNetworkError::PeerNotConnected))
        .await?;

    (async || network_1.is_connected_to(&network_2).await)
        .retry(ExponentialBuilder::default().with_total_delay(Some(Duration::from_secs(10))))
        // When to retry
        .when(|e| matches!(e, TestNetworkError::PeerNotConnected))
        .await?;

    Ok(())
}
