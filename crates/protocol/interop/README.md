# `kona-interop`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-interop"><img src="https://img.shields.io/crates/v/kona-interop.svg?label=kona-interop&labelColor=2a2f35" alt="Kona MPT"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

Core functionality and primitives for the [Interop feature](https://specs.optimism.io/interop/overview.html) of the OP Stack.

## Overview

OP Stack Interop enables secure cross-chain communication between different OP Stack chains. 
This crate provides the foundational types and logic for:

- **Cross-Chain Messaging**: Safe message passing between OP Stack chains
- **Shared Sequencing**: Coordinated block production across multiple chains
- **Dependency Resolution**: Handling of cross-chain transaction dependencies
- **Security Validation**: Ensuring message authenticity and preventing replay attacks

## Key Components

### Supervisor Communication

The interop system relies on a supervisor component that coordinates between chains:
- Tracks dependencies between executing messages
- Validates cross-chain message authenticity
- Ensures atomic execution of dependent transactions

### Message Types

- **Executing Messages**: Cross-chain calls that execute on the destination chain
- **Identifier Types**: Unique identifiers for tracking messages across chains
- **Log Entries**: Structured logs for cross-chain event tracking

## Usage

```rust,ignore
use kona_interop::{ExecutingMessage, SupervisorClient};

// Handle incoming cross-chain message
let message = ExecutingMessage::decode(&data)?;

// Validate message with supervisor
let client = SupervisorClient::new(supervisor_endpoint);
if client.validate_message(&message).await? {
    // Execute cross-chain transaction
}
```

## Security Considerations

Cross-chain communication requires careful security considerations:
- Always validate messages through the supervisor
- Check for replay attacks using message identifiers  
- Ensure proper dependency ordering
- Handle failure cases gracefully

## Features

- `std`: Standard library support (enabled by default)
- `serde`: Serialization support for message types
- `arbitrary`: Fuzzing support via arbitrary trait implementations
