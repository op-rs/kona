# Monorepo Project Proposal

This is a proposal to merge multiple external repositories into
`kona` to create a rust monorepo for Optimism.

## Provenance

Let's rewind the clock to when `kona` was just being started
in the spring of 2024. What little optimism rust types existed
were siloed in applications like `op-reth` and `revm`. Library
code that could have been re-used was unfortunately placed in
an `std` environment that `kona` cannot use because the kona
fault proof program is built for a minimal instruction set.

Effectively, `kona` needed a bunch of Optimism-specific and
slightly-modified Ethereum types that were available in a
`no_std` environment.

As development started, types were defined in `kona`. Mostly
jammed into the `kona-derive` crate for use in derivation, these
types were now being duplicated across a number of rust repos just
to support `no_std`.

Enter `op-alloy`. The first effort to de-duplicate types between
`reth`, `revm`, and `kona` (as well as periphery applications like `magi`).
Rather than keeping types in `kona`, `op-alloy` was introduced as a shared
place for `no_std` compatible optimism rust types. This worked
well as a common place to contribute and decoupled the rapid
development in `kona` from the more stable definition of Optimism
rust types.

Fast forward to 2025, [interop](https://specs.optimism.io/interop/overview.html)
started seriously picking up momentum as a critical project
in the Optimism Rust world. Interop introduces a whole host
of new Optimism-specific types that really should live in a
shared library. But `op-alloy` was already becoming quite large
beyond the minimal, de-duplicated set of types originally intended.

This led to OP folks splitting out Optimism-specific types from
`op-alloy` into a new repo called `maili`. What was not foreseen
was the grievance with yet-another level in dependency chain for
Optimism rust projects. Now, downstream optimism rust projects
could have to import all of these crates just to construct an
OP Stack service:
- `op-alloy`
- `maili`
- `op-revm`
- `op-reth`
- `kona`

And those are just the Optimism-specific crates, let alone
Ethereum crates like `alloy`.


## Why?

The current dependency chain is ever growing.
A small change in `op-alloy` cascades into the following.

- Release `op-alloy` crates
- Update `maili` with `op-alloy` version and changes.
- Release `maili` crates.
- Update `kona` to work with both `op-alloy` and `maili` crates.
- Release `kona`.
- If something breaks in `kona` or downstream consumers, repeat.

To iterate faster without needing to manage releases or change
dependencies into git refs, this doc proposes a monorepo structure
that pulls `maili` into `kona`, while retaining `maili` crate
names **and** versioning.

We propose gone are the days of releasing a single version for
all crates. With a larger, more extensive `kona` monorepo, crates
will need to manage their own semver.

To re-iterate: the key takeaway here is current downstream consumers
of `maili` crates _will not have to change a thing_. Crates pulled
into `kona` will still be published under `maili-` prefixed crate
names. How this is managed while keeping the `kona-` prefix crate
naming consistent is discussed below.


## Proposed Repository Structure

The new repository structure would look as follows.

```
crates/
  proof/
    mpt/
    executor/
    preimage/
    fpvm/
    fpvm-proc/
    proof
    proof-interop/
  protocol/
    derive/
    driver/
    interop/
    genesis/   <-- Has Maili Shadow
    registry/  <-- Has Maili Shadow
    protocol/  <-- Has Maili Shadow
  services/
    rpc/       <-- Has Maili Shadow
    net/
    providers-alloy/
    providers-local/
  utilities/
    serde/     <-- Has Maili Shadow
  cli/
    ..         # TODO
```

> [!INFO]
>
> Crates denoted with `<-- Has Maili Shadow` are ported from `maili`,
> but contain a nested crate with the `maili-` prefix instead of `kona-`
> prefix. These crates re-export their `kona-` equivalent crates. This
> allows downstream users to not need to change their dependencies to
> keep using `maili-` crates! Eventually once the kona monorepo matures,
> and downstream consumers use `kona-` crates instead of `maili-`, these
> can be removed.

#### Maili Shadow Example

Let's look at `crates/protocol/genesis`.

This crate will have a `Cargo.toml` that defines itself as `kona-genesis`.

The contents of the `crates/protocol/genesis` directory will be

```
../genesis/
  README.md
  Cargo.toml  <-- package.name = "kona-genesis"
  src/
    ..   <-- current contents of `maili-genesis`, ported
  maili/
    Cargo.toml  <-- package.name = "maili-genesis"
    src/
      lib.rs  <-- Re-exports `kona-genesis`
```

This structure allows us to seamlessly remain backwards compatible,
while being able to work in the new `kona-` crates without requiring
heavy lifting to support `maili-` crates.
