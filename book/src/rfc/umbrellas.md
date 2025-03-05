# Umbrella Crates

> [!INFO]
>
> TL;DR, this is a proposal to introduce tiny crates inside each
> container directory (e.g. `crates/protocol/`) to re-export all
> crates contained in that directory.

## Context

#### Repository Structure

Kona now has a [monorepo](../archives/monorepo.md) structure that merged
`maili` and `hilo` crates into `kona`. This introduces a number of higher-level
directories that hold a variety of crates themselves. As of the time at which
this document was written the `kona` repository loosely looks like the following.

```
bins/
  -> client/
  -> host/
crates/
  -> protocol/
     -> genesis/
     -> protocol/
     -> derive/
     -> driver/
     -> interop/
     -> registry/
  -> proof/
     -> mpt/
     -> executor/
     -> proof/
     -> proof-interop/
     -> preimage/
     -> std-fpvm/
     -> std-fpvm-proc/
  -> node/
     -> net/
     -> rpc/
     -> engine/
  -> providers/
     -> providers-alloy/
     -> providers-local/
  -> utilities/
     -> serde/
```

Within crates, the `protocol`, `proof`, `node`, `providers`, and `utilities`
directories all contain crates, and are not crates themselves - only directories.

#### Publishing Crates

When crates in `kona` are published, they are all published individually, with no
way to add all kona crates as a dependency. This make discoverability difficult
without accurate and up-to-date documentation, adding overhead.

## Problem

As a monorepo, `kona` will likely have a growing number of crates that will make
it increasingly difficult to discover new `kona` crates and manage `kona` as a
dependency, for downstream consumers.

Additionally, each crate has it's own independent version, which makes it more nuanced
to manage `kona` dependencies and less clear which crate versions are compatible.

## Considered Options

### Single Umbrella Crate

One option that would make `kona` crates the easiest to consume is to provide
a single umbrella crate that lives at the top-level (e.g. `crates/umbrella/Cargo.toml`).

This crate could simply be called `kona` and re-export all crates in the `kona`
monorepo under various feature flags, with propagating `std` and `serde` feature
flags.

#### Tradeoffs

The benefit of this option is providing a single crate to consume all of `kona`,
with the downside of having to manage all the various feature flags and crate
re-exports in the single umbrella crate.

### Grouped Umbrella Crates

In each of the `crates/` sub-directories, provide an umbrella crate that exports
all crates within that subdirectory.

For example, in the `crates/protocol/` subdirectory, an umbrella crate would
re-export all crates in `crates/protocol/`. It could be called
`kona-umbrella-protocol` or some other name to make it easily discoverable.

#### Tradeoffs

While this simplifies updates when adding or removing a re-export, it introduces
`n-1` additional crates as the single umbrella crate, where `n` is the number
of sub-directories in `crates/`. These many umbrella crates also make `kona` less
easily consumed by downstream users of `kona` as opposed to the singular umbrella
crate.

### Top-level Umbrella with Subdirectory Umbrellas (Combined)

Effectively, this option is to combined the previous two options into one.

In this configuration, the top-level umbrella crate could just re-export
each of the umbrella crates in the sub-directory.

#### Tradeoffs

Unfortunately this option now introduces `n + 1` number of crates where `n`
is the number of subdirectories in `crates/`, but it still only requires
updates to the subdirectory umbrellas when a crate is added or removed.

The benefit of this is option is the top-level umbrella crate is very much
simplified, since it only needs to re-export the `n` umbrella crates and not
every crate in the workspace. It also provides the single consumable `kona`
crate for downstream users that greatly simplifies managing `kona` as a
dependency.

## Proposed Solution

???

Looking for comments/thoughts here before landing on a proposed solution.
