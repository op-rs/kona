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
use kona_p2p::{LocalNode, Network, Config};
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
    let network = Network::builder(Config::new(
        RollupConfig::default(),
        disc,
        gossip_addr,
        signer
    )).build().expect("Failed to builder network driver");

    // Starting the network spawns gossip and discovery service
    // handling in a new thread so this is a non-blocking,
    // synchronous operation that does not need to be awaited.
    // If running an RPC server, you'd pass a channel to handle RPC requests as an input to the `start` method
    network.start(None).await.expect("Failed to start network driver");
}
```

[!WARNING]: ###example

### Technical note:

Contrarily to the `op-node`, `kona-node`s don't manually track peer scores. For simplicity, we're relying on the peer score computed by `rust-libp2p`. Since this library doesn't expose all the factors used to compute the peer score (like per topic scores, or the ip-collocation-factor), we're only exposing the total peer score.

See `<https://github.com/libp2p/rust-libp2p/issues/6058>`

### Acknowledgements

Largely based off [magi]'s [p2p module][p2p].

<!-- Links -->

[magi]: https://github.com/a16z/magi
[p2p]: https://github.com/a16z/magi/tree/master/src/network
