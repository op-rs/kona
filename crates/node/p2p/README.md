# `kona-p2p`

A p2p library for the OP Stack.

Contains a gossipsub driver to run discv5 peer discovery and block gossip.

### Example

> **Warning**
>
> Notice, the socket address uses `0.0.0.0`.
> If you are experiencing issues connecting to peers for discovery,
> check to make sure you are not using the loopback address,
> `127.0.0.1` aka "localhost", which can prevent outward facing connections.

```rust,no_run
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use alloy_primitives::address;
use kona_genesis::RollupConfig;
use kona_p2p::{LocalNode, Network};
use libp2p::Multiaddr;
use discv5::enr::CombinedKey;

#[tokio::main]
async fn main() {
    // Construct the Network
    let signer = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099);
    let mut gossip_addr = Multiaddr::from(gossip.ip());
    gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));
    let advertise_ip = IpAddr::V4(Ipv4Addr::UNSPECIFIED);

    let CombinedKey::Secp256k1(k256_key) = CombinedKey::generate_secp256k1() else {
        unreachable!()
    };
    let disc = LocalNode::new(k256_key, advertise_ip, 9097, 9098);
    let network = Network::builder()
        .with_rollup_config(RollupConfig { l2_chain_id: 10, ..Default::default() })
        .with_unsafe_block_signer(signer)
        .with_discovery_address(disc)
        .with_gossip_address(gossip_addr)
        .build()
        .expect("Failed to builder network driver");

    // Starting the network spawns gossip and discovery service
    // handling in a new thread so this is a non-blocking,
    // synchronous operation that does not need to be awaited.
    network.start().await.expect("Failed to start network driver");
}
```

[!WARNING]: ###example

### Acknowledgements

Largely based off [magi]'s [p2p module][p2p].

<!-- Links -->

[magi]: https://github.com/a16z/magi
[p2p]: https://github.com/a16z/magi/tree/master/src/network
