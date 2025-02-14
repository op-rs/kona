# Kona Book

_Documentation for the Kona project._

<a href="https://github.com/op-rs/kona"><img src="https://img.shields.io/badge/GitHub%20Repo-kona-green?logo=github"></a>

> [!WARNING]
>
> `kona` is in active development, and is not yet ready for use in production.
> During development, this book will evolve quickly and may contain inaccuracies.
>
> Please [open an issue][new-issue] if you find any errors or have any suggestions
> for improvements, and also feel free to [contribute][contributing] to the project!


## Introduction

Kona is _the_ monorepo for the Rust Optimism <a href="https://specs.optimism.io/">OP Stack</a>.

Kona provides a suite of libraries and build pipelines for developing verifiable Rust programs targeting
{{#template ../templates/glossary-link.md root=./ ref=fault-proof-vm text=Fault Proof VMs}}.

It is built and maintained by members of [OP Labs][op-labs] as well as open source contributors,
and is licensed under the MIT License.

Kona provides tooling and abstractions around low-level syscalls, memory management, and other common structures that authors of verifiable programs
will need to interact with. It also provides build pipelines for compiling `no_std` Rust programs to a format that can be executed by supported
Fault Proof VM targets.

## Goals of Kona

**1. Composability**

Kona provides a common set of tools and abstractions for developing verifiable Rust programs on top of several supported Fault Proof VM targets. This is done
to ensure that programs written for one supported FPVM can be easily ported to another supported FPVM, and that the ecosystem of programs built on top of these targets
can be easily shared and reused.

**2. Safety**

Through standardization of these low-level system interfaces and build pipelines, Kona seeks to increase coverage over the low-level operations that are
required to build on top of a FPVM.

**3. Developer Experience**

Building on top of custom Rust targets can be difficult, especially when the target is nascent and tooling is not yet mature. Kona seeks to improve this
experience by standardizing and streamlining the process of developing and compiling verifiable Rust programs, targeted at supported FPVMs.

**4. Performance**

Kona is opinionated in that it favors `no_std` Rust programs for embedded FPVM development, for both performance and portability. In contrast with alternative approaches, such
as the [`op-program`][op-program] using the Golang `MIPS64` target, `no_std` Rust programs produce much smaller binaries, resulting in fewer instructions
that need to be executed on the FPVM. In addition, this offers developers more low-level control over interactions with the FPVM kernel, which can be useful
for optimizing performance-critical code.

## Development Status

**Kona is currently in active development, and is not yet ready for use in production.**

## Contributing

Contributors are welcome! Please see the [contributing guide][contributing] for more information.

{{#include ./links.md}}
