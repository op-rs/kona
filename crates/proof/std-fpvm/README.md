# `kona-std-fpvm`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-std-fpvm"><img src="https://img.shields.io/crates/v/kona-std-fpvm.svg?label=kona-std-fpvm&labelColor=2a2f35" alt="Kona Proof SDK"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
<a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>

Platform-specific kernel APIs for [Fault Proof VMs][g-fault-proof-vm].

## Overview

This crate provides a standard library replacement for programs running inside
fault proof VMs. It implements platform-specific kernel APIs that enable 
communication between the proof program and the VM host environment.

## Supported Platforms

### Cannon (MIPS)
- 32-bit MIPS architecture
- Big-endian memory layout
- Custom syscall interface

### Asterisc (RISC-V) 
- 64-bit RISC-V architecture
- Little-endian memory layout
- Standard RISC-V syscalls

## Core Features

### Memory Allocator
Custom allocator optimized for proof programs:
- Deterministic allocation patterns
- Minimal memory overhead
- No system heap dependency

### Panic Handler
Specialized panic handling for fault proofs:
- Writes panic info to designated memory location
- Triggers VM-specific abort sequence
- Preserves debugging information

### I/O Primitives
Low-level I/O for oracle communication:
- File descriptor management
- Preimage oracle read/write
- Hint channel communication

## Usage

This crate is automatically used when building for fault proof targets:

```toml
[dependencies]
kona-std-fpvm = { version = "x.y.z", features = ["cannon"] }
```

In your program, use it as a standard library replacement:

```rust,ignore
#![no_std]
#![no_main]

extern crate kona_std_fpvm;

use kona_std_fpvm::{read, write, exit};

#[no_mangle]
pub extern "C" fn _start() {
    // Your proof program logic
    exit(0);
}
```

## Kernel Interface

### Read (fd 0)
Reads data from the preimage oracle:
```rust,ignore
let mut buffer = [0u8; 32];
let bytes_read = read(0, &mut buffer);
```

### Write (fd 1)
Writes data to the hint channel:
```rust,ignore
let data = b"hint_data";
write(1, data);
```

### Exit
Terminates the program with status:
```rust,ignore
exit(0); // Success
exit(1); // Failure
```

## Platform Differences

### Memory Layout
- **Cannon**: Stack at high memory, grows down
- **Asterisc**: Stack at low memory, grows up

### Syscall Numbers
- **Cannon**: Custom syscall numbers (4000+)
- **Asterisc**: Standard RISC-V syscalls

### Endianness
- **Cannon**: Big-endian
- **Asterisc**: Little-endian

## Security Considerations

- All allocations are deterministic
- No external dependencies or randomness
- Panic messages don't leak sensitive data
- Memory is never freed (grows only)

[g-fault-proof-vm]: https://specs.optimism.io/experimental/fault-proof/index.html#fault-proof-vm
