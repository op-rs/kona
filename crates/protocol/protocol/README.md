## `kona-protocol`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-protocol"><img src="https://img.shields.io/crates/v/kona-protocol.svg" alt="kona-protocol crate"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="MIT License"></a>
<a href="https://rollup.yoga"><img src="https://img.shields.io/badge/Docs-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Docs" /></a>


Core protocol types and utilities for the OP Stack.

This crate provides the foundational types, constants, and methods necessary for 
L2 derivation and batch submission in OP Stack chains. It serves as the protocol 
layer that enables rollup nodes to transform L1 data into L2 blocks.

## Features

- **Batch Processing**: Support for both single and span batch formats
- **Frame Handling**: Low-level frame parsing and channel assembly
- **Block Conversions**: L1/L2 block info types and transformations  
- **Compression**: Built-in Brotli decompression for batch data
- **No-std Compatible**: Core functionality works in `no_std` environments

## Key Components

### Batches
The fundamental unit of L2 transaction data:
- `SingleBatch`: Traditional format for single L2 blocks
- `SpanBatch`: Optimized format supporting multiple L2 blocks with compression
- `BatchReader`: Unified interface for reading any batch type

### Frames & Channels
Low-level data transport:
- `Frame`: Basic data unit with sequencing
- `Channel`: Ordered collection of frames containing batch data
- Built-in support for Fjord hardfork channel size limits

### Block Information
- `BlockInfo`: Standardized L1/L2 block metadata
- `L2BlockInfo`: L2-specific block details including L1 origin
- Conversion utilities for Alloy block types

## Usage Example

```rust
use kona_protocol::{BatchReader, Frame, Channel};

// Decode a frame from raw data
let frame = Frame::decode(&raw_frame_data)?;

// Add frame to a channel
let mut channel = Channel::new(channel_id, origin);
channel.add_frame(frame)?;

// Read batch from completed channel  
if channel.is_ready() {
    let batch = BatchReader::new(channel.bytes()).read_batch()?;
    // Process batch to derive L2 block attributes
}
```

## Safety & Security

This crate handles critical consensus-level protocol logic. When working with batch 
data, always validate against a trusted L1 source and use appropriate error handling.

## Feature Flags

- `std` (default): Enables standard library features and additional error context
- `test-utils`: Provides testing utilities for downstream crates

### Provenance

This code was initially ported from [kona-primitives] to [op-alloy] as part of maili migrations.

[kona-primitives]: https://github.com/ethereum-optimism/kona/tree/main/crates/kona-primitives
[op-alloy]: https://github.com/alloy-rs/op-alloy
