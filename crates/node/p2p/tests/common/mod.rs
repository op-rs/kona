//! Shared code for integration tests.

use alloy_primitives::Address;
use kona_genesis::RollupConfig;
use kona_p2p::{Behaviour, BlockHandler, GossipDriver};
use libp2p::{Multiaddr, StreamProtocol, SwarmBuilder, identity::Keypair, multiaddr::Protocol};
use std::net::Ipv4Addr;

/// Helper function to create a new gossip driver instance.
pub(crate) fn gossip_driver(port: u16) -> GossipDriver {
    let timeout = std::time::Duration::from_secs(60);
    let mut addr = Multiaddr::empty();
    addr.push(Protocol::Ip4(Ipv4Addr::UNSPECIFIED));
    addr.push(Protocol::Tcp(port));

    // Use the default `kona_p2p` config for the gossipsub protocol.
    let config = kona_p2p::default_config();

    let keypair = Keypair::generate_secp256k1();

    // Construct a Behaviour instance
    let unsafe_block_signer = Address::default();
    let (_, unsafe_block_signer_recv) = tokio::sync::watch::channel(unsafe_block_signer);
    let handler = BlockHandler::new(
        RollupConfig { l2_chain_id: 10, ..Default::default() },
        unsafe_block_signer_recv,
    );
    let behaviour = Behaviour::new(keypair.public(), config, &[Box::new(handler.clone())])
        .expect("creates behaviour");

    // Create a sync request/response protocol handler.
    // We are accepting inbound sync requests to the `payload_by_number` protocol.
    // Since this is only a mock sync protocol, we are not supporting outbound sync requests.
    let mut sync_handler = behaviour.sync_req_resp.new_control();
    let protocol = format!("/opstack/req/payload_by_number/{}/0/", 10);
    let sync_protocol_name =
        StreamProtocol::try_from_owned(protocol).expect("error creating sync protocol");
    let sync_protocol =
        sync_handler.accept(sync_protocol_name).expect("error accepting sync protocol");

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            |i: &Keypair| libp2p::noise::Config::new(i),
            libp2p::yamux::Config::default,
        )
        .expect("adds tcp config to swarm")
        .with_behaviour(|_| behaviour)
        .expect("adds behaviour to swarm")
        .with_swarm_config(|c| c.with_idle_connection_timeout(timeout))
        .build();

    GossipDriver::new(swarm, addr, Some(2), handler, sync_handler, sync_protocol)
}
