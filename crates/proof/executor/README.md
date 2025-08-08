# `kona-executor`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-executor"><img src="https://img.shields.io/crates/v/kona-executor.svg?label=kona-executor&labelColor=2a2f35" alt="Kona Stateless Executor"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

A `no_std` implementation of a stateless block executor for the OP Stack.

## Overview

The executor is responsible for processing L2 blocks and computing the resulting
state root. It operates in a stateless manner, fetching required state data
on-demand through the `TrieDB` interface backed by [`kona-mpt`](../mpt).

## Key Features

- **Stateless Execution**: Fetches only required state data during execution
- **Fault Proof Compatible**: Designed for use in fault proof programs
- **No-std Support**: Runs in constrained environments without heap allocation
- **OP Stack Native**: Handles all OP Stack-specific transaction types and opcodes

## Architecture

The executor processes blocks through several stages:

1. **State Preparation**: Loads the pre-state root and prepares the trie database
2. **Transaction Execution**: Processes each transaction in the block sequentially
3. **State Updates**: Applies state changes including storage, balance, and nonce updates
4. **Receipt Generation**: Creates transaction receipts with accurate gas usage
5. **State Root Computation**: Calculates the post-execution state root

## Usage

```rust,ignore
use kona_executor::{BlockExecutor, TrieDB};
use kona_mpt::TrieProvider;

// Initialize the state database
let provider = TrieProvider::new(/* ... */);
let mut trie_db = TrieDB::new(provider);

// Create executor with L2 chain config
let executor = BlockExecutor::new(&chain_config, &mut trie_db);

// Execute a block
let header = executor.execute_block(&block)?;
let state_root = header.state_root;
```

## State Management

The executor uses a copy-on-write approach for state modifications:
- Original state remains untouched until committed
- All changes are tracked in memory during execution
- State root is computed from the modified trie

## Performance Considerations

- Minimize state access by batching reads where possible
- Use caching layers for frequently accessed state
- Consider prefetching state data for known access patterns

## Security

- Always validate block headers before execution
- Ensure state data comes from trusted sources
- Verify state roots match expected values after execution
