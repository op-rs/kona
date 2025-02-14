# System Config

The system configuration is a set of configurable chain parameters
defined in a contract on L1. These parameters can be changed through
the system config contract, emitting events that are picked up by
the [rollup node derivation process][derivation]. To dive deeper
into the System Config, visit the
[OP Stack Specifications][system-config-specs].

## `SystemConfig` Type

The [`SystemConfig`][system-config] type is defined in
[`maili-genesis`][genesis].

Parameters defined in the [`SystemConfig`][system-config] are expected to be
updated through L1 receipts, using the [`update_with_receipts`][update] method.

## Holocene Updates

The [Holocene Hardfork][holocene] introduced an update to the
[`SystemConfig`][system-config] type, adding EIP-1559 parameters to the config.

The [`SystemConfig`][system-config] type in [`maili-genesis`][genesis] provides
a method called [`eip_1559_params`][eip] that returns the EIP-1559 parameters
encoded as a [`B64`][b64].


<!-- Links -->

{{#include ../../links.md}}
