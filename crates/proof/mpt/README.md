# `kona-mpt`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-mpt"><img src="https://img.shields.io/crates/v/kona-mpt.svg?label=kona-mpt&labelColor=2a2f35" alt="Kona MPT"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

A `no_std` compatible Merkle Patricia Trie implementation optimized for Ethereum state proofs.

## Overview

This crate provides a recursive, in-memory implementation of Ethereum's hexary Merkle Patricia Trie (MPT).
It's designed specifically for stateless execution within fault proof programs, where minimal
code size and deterministic behavior are critical.

## Features

- **Core Operations**: Retrieval, insertion, deletion, and root computation
- **Stateless Design**: Fetches node data on-demand via `TrieProvider`
- **Memory Efficient**: Nodes are loaded only when accessed
- **RLP Encoding**: Full support for Ethereum's trie node encoding
- **No-std Compatible**: Works in constrained environments

## Architecture

### Trie Structure

The MPT uses a hexary (16-way) tree structure with three node types:
- **Branch Nodes**: 16 children plus optional value
- **Extension Nodes**: Shared path prefix optimization
- **Leaf Nodes**: Terminal nodes containing values

### Key Encoding

Keys are nibblized (split into 4-bit values) and encoded with a prefix:
- Even-length paths: `0x00` prefix
- Odd-length paths: `0x1` prefix with first nibble
- Leaf nodes: Additional `0x20` flag

## Usage

```rust,ignore
use kona_mpt::{TrieNode, TrieProvider};

// Create a trie provider (implements data fetching)
let provider = MyTrieProvider::new();

// Open a trie at a specific root
let mut trie = TrieNode::new(root_hash);

// Retrieve a value
let value = trie.get(&key, &provider)?;

// Insert a new value
trie.insert(&key, value, &provider)?;

// Compute the new root
let new_root = trie.root();
```

## Stateless Execution

This implementation is optimized for stateless execution:

1. **Lazy Loading**: Nodes are fetched only when traversed
2. **Proof Generation**: Access patterns naturally generate witness data
3. **Deterministic**: Same inputs always produce same outputs
4. **Minimal Dependencies**: Reduces code size in proof programs

## Integration with kona-executor

The [`kona-executor`](../executor) uses this trie implementation for:
- Loading account states
- Reading contract storage
- Computing state roots after execution
- Generating storage proofs

## Security Considerations

- Always validate trie roots against trusted sources
- Ensure the `TrieProvider` only serves authentic data
- Be aware of potential DoS via deep trie structures
- Consider gas costs when traversing large tries
