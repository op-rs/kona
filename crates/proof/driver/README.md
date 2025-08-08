# `kona-driver`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-driver"><img src="https://img.shields.io/crates/v/kona-driver.svg?label=kona-driver&labelColor=2a2f35" alt="Kona Driver"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

A `no_std` compatible driver for the OP Stack derivation pipeline.

## Overview

The driver orchestrates the entire L2 block derivation process, coordinating between
data retrieval, derivation stages, and block execution. It serves as the main entry
point for running the derivation pipeline in various contexts.

## Core Responsibilities

### Pipeline Management
- Initializes and advances derivation pipeline stages
- Handles pipeline resets and error recovery
- Manages state transitions between L1 origins

### Block Production
- Derives L2 attributes from L1 data
- Validates derived attributes against chain rules
- Coordinates with the execution engine

### Safety Guarantees
- Ensures correct ordering of L2 blocks
- Validates timestamp and sequence constraints
- Handles reorgs and invalid block detection

## Architecture

The driver integrates several components:

```text
L1 Data Source
     ↓
Derivation Pipeline ← Driver coordinates all stages
     ↓
L2 Attributes
     ↓
Execution Engine
```

## Usage

```rust,ignore
use kona_driver::{Driver, DriverConfig};

// Configure the driver
let config = DriverConfig {
    chain_config,
    l1_provider,
    l2_provider,
    attributes_builder,
};

// Create and run the driver
let mut driver = Driver::new(config);

// Advance the pipeline to derive next block
let attributes = driver.advance_to_next_block().await?;

// Execute the derived block
let header = driver.execute_attributes(attributes).await?;
```

## Pipeline States

The driver manages several pipeline states:

1. **Idle**: Waiting for new L1 data
2. **Deriving**: Processing L1 data to create L2 attributes  
3. **Executing**: Running derived blocks through execution engine
4. **Syncing**: Catching up to L2 chain tip

## Error Handling

The driver implements comprehensive error handling:
- **Temporary Errors**: Retry with backoff
- **Critical Errors**: Reset pipeline to safe state
- **Invalid Blocks**: Mark as invalid and continue
- **Reorgs**: Reset to common ancestor

## Integration Points

### With Fault Proofs
The driver can run within fault proof programs:
- Deterministic execution path
- Minimal external dependencies
- Reproducible state transitions

### With Rollup Nodes
Also suitable for full node implementations:
- Real-time block production
- P2P block distribution
- RPC endpoint serving

## Safety and Correctness

- All derived blocks follow consensus rules
- State transitions are deterministic
- Invalid data cannot corrupt the chain
- Comprehensive test coverage ensures reliability
