# Kona Book

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://img.shields.io/github/actions/workflow/status/op-rs/kona/rust_ci.yaml?style=flat&labelColor=1C2C2E&label=ci&color=BEC5C9&logo=GitHub%20Actions&logoColor=BEC5C9" alt="CI"></a>
   <a href="https://codecov.io/gh/op-rs/kona"><img src="https://img.shields.io/codecov/c/gh/op-rs/kona?style=flat&labelColor=1C2C2E&logo=Codecov&color=BEC5C9&logoColor=BEC5C9" alt="Codecov"></a>
   <a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=googledocs&label=license&logoColor=BEC5C9" alt="License"></a>
   <a href="https://op-rs.github.io/kona"><img src="https://img.shields.io/badge/Book-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Book"></a>

> [!WARNING]
>
> This book may contain inaccuracies as it evolves.
>
> Please [open an issue][new-issue] if you find any errors or have any suggestions
> for improvements, and also feel free to [contribute][contributing] to the project!


## Introduction

Originally a suite of portable implementations of the OP Stack rollup state transition,
Kona has been extended to be _the monorepo_ for <a href="https://specs.optimism.io/">OP Stack</a>
types, components, and services built in Rust. Kona provides an ecosystem of extensible, low-level
crates that compose into components and services required for the OP Stack.

Protocol crates are `no_std` compatible for use within the Fault Proof. Types defined in these
libraries are shared by other components of the OP Stack as well including the rollup node.

Proof crates are available for developing verifiable Rust programs targeting
{{#template ../templates/glossary-link.md root=./ ref=fault-proof-vm text=Fault Proof VMs}}.
These libraries provide tooling and abstractions around low-level syscalls, memory management,
and other common structures that authors of verifiable programs will need to interact with.
It also provides build pipelines for compiling `no_std` Rust programs to a format that can be
executed by supported Fault Proof VM targets.

Kona is built and maintained by open source contributors and is licensed under the MIT License.

## Goals of Kona

**1. Composability**

Kona provides a common set of tools and abstractions for developing verifiable Rust programs
on top of several supported Fault Proof VM targets. This is done to ensure that programs
written for one supported FPVM can be easily ported to another supported FPVM, and that the
ecosystem of programs built on top of these targets can be easily shared and reused.

**2. Safety**

Through standardization of these low-level system interfaces and build pipelines, Kona seeks
to increase coverage over the low-level operations that are required to build on top of a FPVM.

**3. Developer Experience**

Building on top of custom Rust targets can be difficult, especially when the target is
nascent and tooling is not yet mature. Kona seeks to improve this experience by standardizing
and streamlining the process of developing and compiling verifiable Rust programs, targeted
at supported FPVMs.

**4. Performance**

Kona is opinionated in that it favors `no_std` Rust programs for embedded FPVM development,
for both performance and portability. In contrast with alternative approaches, such as the
[`op-program`][op-program] using the Golang `MIPS64` target, `no_std` Rust programs produce
much smaller binaries, resulting in fewer instructions that need to be executed on the FPVM.
In addition, this offers developers more low-level control over interactions with the FPVM
kernel, which can be useful for optimizing performance-critical code.

## Contributing

Contributors are welcome! Please see the [contributing guide][contributing] for more information.

{{#include ./links.md}}
