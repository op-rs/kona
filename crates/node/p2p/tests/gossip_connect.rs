//! Test to connecting to a node via its enr.

mod common;

#[tokio::test]
async fn test_unknown_peer_connect_fails() {
    let mut driver = common::gossip_driver(4003);
    assert!(driver.listen().await.is_ok());

    let mut driver_2 = common::gossip_driver(4004);
    assert!(driver_2.listen().await.is_ok());

    let err = driver.swarm.dial(*driver_2.local_peer_id()).unwrap_err();
    assert!(matches!(err, libp2p::swarm::DialError::NoAddresses));
}

#[tokio::test]
async fn test_connect_to_peer() {
    let mut driver = common::gossip_driver(4005);
    assert!(driver.listen().await.is_ok());

    let mut driver_2 = common::gossip_driver(4006);
    assert!(driver_2.listen().await.is_ok());

    assert!(driver.swarm.dial(driver_2.addr).is_ok());
    assert_eq!(driver.connected_peers(), 0);
}
