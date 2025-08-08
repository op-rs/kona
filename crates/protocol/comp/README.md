## `kona-comp`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-comp"><img src="https://img.shields.io/crates/v/kona-comp.svg" alt="kona-comp crate"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="MIT License"></a>
<a href="https://rollup.yoga"><img src="https://img.shields.io/badge/Docs-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Docs" /></a>

Compression types and utilities for the OP Stack.

## Overview

This crate provides compression functionality used throughout the OP Stack to reduce
data costs and improve efficiency. It implements the compression schemes used in
various parts of the protocol, particularly for batch submission and data availability.

## Compression Algorithms

### Brotli Compression

The primary compression algorithm used for batch data:
- Highly efficient for text-like transaction data
- Configurable compression levels for speed/size tradeoffs
- Native support in most environments

### Zlib Compression

Alternative compression for specific use cases:
- Legacy compatibility with older systems
- Lower CPU overhead for time-sensitive operations
- Streaming compression support

## Usage

```rust,ignore
use kona_comp::{compress_brotli, decompress_brotli};

// Compress batch data
let compressed = compress_brotli(&batch_data, 6)?; // Level 6 compression

// Decompress on the receiving end
let original = decompress_brotli(&compressed)?;
```

## Performance Considerations

- Compression level selection impacts both size and CPU usage
- Higher levels (7-11) provide better compression but slower processing
- Level 6 is typically optimal for batch submission
- Consider network costs vs computational costs when selecting levels

## Features

- `std`: Standard library support (enabled by default)
- `no_std`: Core compression functionality without std
- `async`: Async compression/decompression support
