# `kona-preimage`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-preimage"><img src="https://img.shields.io/crates/v/kona-preimage.svg?label=kona-preimage&labelColor=2a2f35" alt="Kona Preimage ABI client"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

High-level abstractions for the Preimage Oracle ABI used in OP Stack fault proofs.

## Overview

The Preimage Oracle is a critical component of the fault proof system that enables
deterministic access to external data during proof execution. This crate provides
a clean, type-safe interface over the low-level [Preimage Oracle ABI][preimage-abi-spec].

## Architecture

The crate is split into two main components:

### Client Side (`no_std`)
Used within fault proof programs to request data:
- Synchronous API for requesting preimages
- Type-safe hint system for guided data fetching
- Zero-allocation design for constrained environments

### Host Side (`async`)
Used by the host program to serve data requests:
- Async handlers for external data fetching
- Support for multiple data sources (RPC, disk, etc.)
- Efficient caching and request batching

## Preimage Types

The oracle supports multiple preimage types:

- **Keccak256**: Standard hash preimages
- **SHA256**: Alternative hash function support
- **Blob Point**: EIP-4844 blob commitment preimages
- **Precompile**: Results from Ethereum precompiles
- **Local**: Application-specific data

## Usage

### Client Example

```rust,ignore
use kona_preimage::{PreimageOracle, HintType};

// Request a preimage by its hash
let preimage = oracle.get_preimage(hash)?;

// Send a hint to guide the host's data fetching
oracle.hint(HintType::L1Block { number: 12345 })?;
```

### Host Example

```rust,ignore
use kona_preimage::{PreimageServer, PreimageProvider};

// Implement a custom provider
struct MyProvider;

impl PreimageProvider for MyProvider {
    async fn get_preimage(&self, key: PreimageKey) -> Result<Vec<u8>> {
        // Fetch from RPC, database, etc.
    }
}

// Run the server
let server = PreimageServer::new(MyProvider);
server.run().await?;
```

## Communication Protocol

The client and host communicate via:
- **Read/Write file descriptors**: For data transfer
- **Hint file descriptor**: For optimization hints
- **Length-prefixed messages**: Ensuring message boundaries

## Security Considerations

- The oracle is trusted to provide correct data
- All preimages must be verifiable by their commitment
- Hints are optimization-only and don't affect correctness
- The host must validate all requests before serving data

[preimage-abi-spec]: https://specs.optimism.io/experimental/fault-proof/index.html#pre-image-oracle
