# `kona-proof`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-proof"><img src="https://img.shields.io/crates/v/kona-proof.svg?label=kona-proof&labelColor=2a2f35" alt="Kona Proof SDK"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

High-level OP Stack state transition proof SDK.

## Overview

`kona-proof` provides a complete SDK for generating and verifying state transition
proofs for OP Stack chains. It orchestrates the entire proof generation process,
from fetching L1 data to producing the final state root.

## Core Components

### Boot Process
Initializes the proof program with:
- Chain configuration and genesis state
- L1/L2 block targets
- Oracle configuration for data fetching

### L1 Data Fetching
Retrieves required L1 data including:
- L1 blocks and receipts
- Batcher transactions containing L2 data
- Blob data (post-EIP4844)
- System configuration updates

### L2 Derivation
Processes L1 data to derive L2 blocks:
- Decodes batched transaction data
- Applies sequencing rules
- Generates L2 block attributes

### State Execution
Executes derived L2 blocks:
- Processes all transactions
- Updates state trie
- Computes new state root

## Usage Example

```rust,ignore
use kona_proof::{ProofProgram, BootInfo, CachingOracle};

// Initialize boot information
let boot_info = BootInfo::from_env()?;

// Create the proof program
let oracle = CachingOracle::new(preimage_oracle);
let program = ProofProgram::new(&boot_info, oracle);

// Run the complete proof
let output = program.run()?;

// The output contains the proven L2 state root
assert_eq!(output.state_root, expected_root);
```

## Proof Pipeline

```text
L1 Chain Data → Derivation → L2 Blocks → Execution → State Root
      ↓                                        ↓
   Oracle Requests                    State Updates
```

## Oracle Interface

The proof program relies on an oracle for external data:
- **Preimage Oracle**: Provides data by hash
- **Hint System**: Guides data fetching
- **Caching Layer**: Reduces redundant requests

## Security Model

- **Deterministic Execution**: Same inputs always produce same outputs
- **Stateless Verification**: No persistent state between proofs
- **Data Availability**: All required data must be accessible
- **Fault Isolation**: Invalid inputs result in proof failure

## Integration

This SDK is designed to work with fault proof VMs:
- Cannon (MIPS)
- Asterisc (RISC-V)
- Any VM implementing the required host functions

## Features

- `std`: Standard library support
- `tracing`: Detailed execution logging
- `metrics`: Performance instrumentation
