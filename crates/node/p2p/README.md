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
use kona_p2p::driver::NetworkDriver;

// Build the network driver.
let signer = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
let driver = NetworkDriver::builder()
    .with_chain_id(10) // op mainnet chain id
    .with_unsafe_block_signer(signer)
    .with_gossip_addr(socket)
    .build()
    .expect("Failed to builder network driver");

// Call `.start()` on the driver.
driver.start().expect("Failed to start network driver");

println!("NetworkDriver started.");
```

[!WARNING]: ###example

### Acknowledgements

Largely based off [magi]'s [p2p module][p2p].

<!-- Links -->

[magi]: https://github.com/a16z/magi
[p2p]: https://github.com/a16z/magi/tree/master/src/network
