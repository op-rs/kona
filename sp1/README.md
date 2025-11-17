# `sp1`

This directory contains an integration of [SP1](https://github.com/succinctlabs/sp1) zero-knowledge proof capabilities into Kona, enabling validity proofs for OP Stack state transitions. This integration is derived from the [OP-Succinct](https://github.com/succinctlabs/op-succinct) project.

> **⚠️ Experimental**: The SP1 fault proof integration is currently experimental and under active development. It is not yet recommended for production use.

## Overview

The SP1 integration provides zkVM-based fault proofs for the OP Stack, allowing verifiable state transitions to be proven on-chain. This enables trustless bridging and enhanced security for rollup chains.

## Structure

### Programs (`programs/`)

zkVM programs that execute inside the SP1 prover:

- **`range`**: Verifies OP Stack state transitions across a range of L2 blocks with Ethereum DA. Generates proofs for multi-block execution that can be verified on-chain.
- **`aggregation`**: Aggregates multiple range program proofs into a single proof, enabling efficient verification of longer block ranges.

### Crates (`crates/`)

Supporting libraries for the SP1 fault proof system:

- **`build`**: Build utilities for compiling SP1 programs
- **`client`**: Client-side utilities and types for witness execution in the zkVM
- **`elfs`**: Management and references to compiled ELF binaries
- **`ethereum`**: Ethereum-specific data availability utilities
  - `client/`: Client-side Ethereum DA utilities
  - `host/`: Host-side Ethereum DA witness generation
- **`host`**: Host utilities for witness generation, proof orchestration, and preimage serving
- **`proof`**: High-level proof generation utilities and workflows

### ELF Binaries (`elf/`)

Compiled ELF binaries for the zkVM programs, used by the prover:

- **`aggregation-elf`**: Compiled aggregation program
- **`range-elf-bump`**: Range program with built with a bump allocator
- **`range-elf-embedded`**: Range program built with the sp1-zkvm "embedded" allocator

## Usage

The SP1 integration follows the same fault proof workflow as the native Kona implementation, but generates cryptographic proofs of execution:

1. **Range Proof Generation**: The `range` program executes state transitions for a block range in the zkVM, producing a validity proof
2. **Proof Aggregation**: The `aggregation` program combines multiple range proofs into a single proof for efficient on-chain verification
3. **On-chain Verification**: Proofs are submitted to the dispute game contract and verified on L1

## Building

Build utilities are provided in the `build` crate. Programs can be compiled for the zkVM target using the SP1 toolchain.

## Dependencies

This integration depends on:
- SP1 SDK and zkVM runtime
- Core Kona libraries (`kona-proof`, `kona-derive`, `kona-executor`, etc.)
- Alloy and OP-Alloy for Ethereum types
- RocksDB for witness data storage

## License and Attribution

This implementation is derived from [OP-Succinct](https://github.com/succinctlabs/op-succinct)
by Succinct Labs and incorporates code licensed under the MIT License and Apache License 2.0.
Significant modifications have been made to integrate with the Kona monorepo architecture.

See [LICENSE-THIRD-PARTY](./LICENSE-THIRD-PARTY) for full license details and attribution.
