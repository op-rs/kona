# `kona-proof-interop`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-proof-interop"><img src="https://img.shields.io/crates/v/kona-proof-interop.svg?label=kona-proof-interop&labelColor=2a2f35" alt="Kona Proof SDK"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

OP Stack state transition proof SDK with cross-chain interoperability support.

## Overview

This crate extends [`kona-proof`](../proof/) with interoperability features, enabling
proof generation for cross-chain operations within the OP Stack superchain. It supports
proving state transitions that involve dependencies and interactions between multiple chains.

## Key Features

### Cross-Chain Dependencies
- Proves execution of messages from other chains
- Validates cross-chain state dependencies
- Ensures atomic execution across chains

### Supervisor Integration
- Communicates with supervisor for dependency tracking
- Validates cross-chain message authenticity
- Coordinates multi-chain proof generation

### Enhanced Oracle
- Extended hint types for cross-chain data
- Multi-chain state fetching capabilities
- Optimized cross-chain data retrieval

## Architecture

The interop proof system extends the standard proof flow:

```text
Standard Proof Pipeline
         +
Cross-Chain Messages
         +
Supervisor Validation
         ↓
    Interop Proof
```

## Usage

```rust,ignore
use kona_proof_interop::{InteropProofProgram, InteropBootInfo};
use kona_interop::SupervisorClient;

// Initialize with interop configuration
let boot_info = InteropBootInfo::from_env()?;
let supervisor = SupervisorClient::new(supervisor_endpoint);

// Create interop-aware proof program
let program = InteropProofProgram::new(
    &boot_info,
    oracle,
    supervisor,
);

// Run proof with cross-chain validation
let output = program.run()?;

// Output includes cross-chain execution results
assert!(output.cross_chain_messages_valid);
```

## Cross-Chain Message Flow

1. **Message Initiation**: Source chain emits cross-chain message
2. **Dependency Tracking**: Supervisor records message dependencies
3. **Target Execution**: Destination chain executes message
4. **Proof Generation**: Proves valid execution with dependencies
5. **Validation**: Supervisor validates cross-chain consistency

## Security Model

### Additional Validations
- Cross-chain message authentication
- Dependency ordering enforcement
- Replay protection across chains
- Atomic failure handling

### Trust Assumptions
- Supervisor provides correct dependency data
- All participating chains follow protocol
- Message relay is eventually consistent

## Differences from Standard Proofs

### Enhanced Boot Info
- Multi-chain configuration
- Cross-chain message lists
- Dependency specifications

### Extended Execution
- Validates incoming messages
- Tracks outgoing messages
- Ensures dependency satisfaction

### Additional Outputs
- Cross-chain execution receipts
- Dependency resolution proofs
- Message authentication data

## Performance Considerations

- Cross-chain proofs require more data fetching
- Dependency resolution may add latency
- Consider batching related cross-chain operations
- Cache supervisor responses when possible
