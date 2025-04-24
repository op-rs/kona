//! Shared code for integration tests.

use alloy_primitives::Address;
use kona_p2p::{Behaviour, BlockHandler, GossipDriver};
use libp2p::{Multiaddr, SwarmBuilder, identity::Keypair, multiaddr::Protocol};
use std::net::Ipv4Addr;

/// Helper function to create a new gossip driver instance.
pub(crate) fn gossip_driver(port: u16) -> GossipDriver {
    let chain_id = 10;
    let timeout = std::time::Duration::from_secs(60);
    let mut addr = Multiaddr::empty();
    addr.push(Protocol::Ip4(Ipv4Addr::new(0, 0, 0, 0)));
    addr.push(Protocol::Tcp(port));

    // Use the default `kona_p2p` config for the gossipsub protocol.
    let config = kona_p2p::default_config();

    // Construct a Behaviour instance
    let unsafe_block_signer = Address::default();
    let (_, unsafe_block_signer_recv) = tokio::sync::watch::channel(unsafe_block_signer);
    let handler = BlockHandler::new(chain_id, unsafe_block_signer_recv);
    let behaviour =
        Behaviour::new(config, &[Box::new(handler.clone())]).expect("creates behaviour");

    // Construct the
    let keypair = Keypair::generate_secp256k1();
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

    GossipDriver::new(swarm, addr, Some(2), handler)
}
