# `kona-p2p`

A peer-to-peer networking library for the OP Stack, providing decentralized node communication and coordination for Optimism rollups.

## Features

- **Peer Discovery**: Automatic discovery of network peers using Ethereum's Discv5 protocol
- **Block Gossip**: Efficient propagation of blocks and network payloads via GossipSub mesh networking  
- **Connection Management**: Intelligent peer connection gating with rate limiting and IP filtering
- **RPC Interface**: Administrative JSON-RPC API for network monitoring and control
- **Metrics**: Prometheus-compatible observability and monitoring (optional)

## Architecture

The library is organized into four main modules:

- **`gossip`**: GossipSub-based block propagation and validation using libp2p
- **`discv5`**: Peer discovery service using Ethereum's Discv5 distributed hash table
- **`rpc`**: Administrative RPC API for network status and peer management  
- **`metrics`**: Observability and monitoring capabilities

## Quick Start

```rust
use kona_p2p::{GossipDriverBuilder, Discv5Builder};
use libp2p_identity::Keypair;
use std::net::Ipv4Addr;

// Create a keypair for the node
let keypair = Keypair::generate_secp256k1();

// Build the gossip driver
let gossip_driver = GossipDriverBuilder::new()
    .with_keypair(keypair.clone())
    .with_addr("127.0.0.1:9000".parse()?)
    .build()?;

// Build the discovery service  
let discv5_driver = Discv5Builder::new()
    .with_keypair(keypair)
    .with_listen_addr(Ipv4Addr::LOCALHOST, 9001)
    .build()?;
```

## Network Protocol

The library implements the OP Stack networking protocol, which consists of:

1. **Discovery Layer**: Uses Discv5 to maintain a distributed hash table of network peers
2. **Transport Layer**: TCP connections secured with libp2p Noise encryption  
3. **Application Layer**: GossipSub mesh for efficient message propagation

### Message Types

The primary message type is [`OpNetworkPayloadEnvelope`], which contains:
- Block payloads for consensus coordination
- Network metadata and validation information
- Cryptographic signatures for message authenticity

### Connection Management

The library includes sophisticated connection management:
- **Rate Limiting**: Prevents connection flooding attacks
- **IP Filtering**: Blocks malicious or unwanted IP ranges
- **Peer Protection**: Maintains connections to important peers
- **Automatic Pruning**: Removes stale or poor-quality connections

## Configuration

Key configuration options include:

- **Mesh Parameters**: Control GossipSub mesh topology (D, D_low, D_high, D_lazy)
- **Discovery Settings**: Bootstrap nodes, query intervals, and table maintenance
- **Connection Limits**: Maximum peers, connection timeouts, and rate limits
- **Validation Rules**: Message validation thresholds and scoring parameters

## Technical Notes

**Peer Scoring**: Unlike the reference `op-node`, `kona-node` relies on libp2p's built-in peer scoring rather than implementing custom scoring. This simplifies the implementation while maintaining network health through proven scoring mechanisms.

**Security**: The library implements multiple layers of protection against common P2P attacks including eclipse attacks, sybil attacks, and denial-of-service attempts.

## Observability

With the `metrics` feature enabled, the library exports Prometheus-compatible metrics for:
- Peer connection counts and quality
- Message propagation statistics  
- Discovery service performance
- Network health indicators

## Compatibility

This implementation is compatible with the OP Stack networking protocol and can interoperate with:
- `op-node` (reference implementation)
- Other OP Stack rollup nodes
- Ethereum consensus layer clients (for discovery)

## Acknowledgements

Largely based off [magi]'s [p2p module][p2p], adapted for the `kona` ecosystem with additional features and OP Stack specific optimizations.

<!-- Links -->

[magi]: https://github.com/a16z/magi
[p2p]: https://github.com/a16z/magi/tree/master/src/network
[`OpNetworkPayloadEnvelope`]: https://docs.rs/op-alloy-rpc-types-engine/latest/op_alloy_rpc_types_engine/struct.OpNetworkPayloadEnvelope.html
