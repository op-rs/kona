# Kona Proof SDK

Welcome to the Kona Proof SDK, a powerful set of libraries designed
from first principles to build proofs with the OP Stack STF on top
of the OP Stack's FPVMs and other verifiable backends like [SP-1][sp-1],
[Risc0][rzero], [Intel TDX][tdx], and [AMD SEV-SNP][sev-snp]. At its
core, Kona is built on the principles of modularity, extensibility,
and developer empowerment.

## A Foundation of Flexibility

The kona repository is more than a fault proof program for the OP Stack
— it's an ecosystem of interoperable components, each crafted with
reusability and extensibility as primary goals. While we provide
{{#template ../../templates/glossary-link.md root=./ ref=fault-proof-vm text=Fault Proof VM}} and "online" backends
for key components like `kona-derive` and `kona-executor`, the true
power of `kona` lies in its adaptability.

## Extend Without Forking

One of Kona's standout features is its ability to support custom
features and data sources without requiring you to fork the entire
project. Through careful use of Rust's powerful trait system and
abstract interfaces, we've created a framework that allows you to
plug in your own features and ideas seamlessly.

## What You'll Learn

In this section of the developer book, we'll dive deep into the Kona SDK, covering:
* **Building on the FPVM Backend**: Learn how to leverage the Fault Proof VM tooling to create your own fault proof programs.
* **Creating Custom Backends**: Discover the process of designing and implementing your own backend to run `kona-client` or a variation of it on different targets.
* **Extending Core Components**: Explore techniques for creating new constructs that integrate smoothly with crates like `kona-derive` and `kona-executor`.

Whether you're looking to use Kona as-is, extend its functionality, or create entirely new programs based on its libraries,
this guide is intended to provide you with the knowledge and tools you need to succeed.

[sp-1]: https://github.com/succinct-labs/sp-1
[rzero]: https://github.com/risc0/risc0
[tdx]: https://www.intel.com/content/www/us/en/developer/tools/trust-domain-extensions/documentation.html
[sev-snp]: https://www.amd.com/en/developer/sev.html

{{#include ../links.md}}
