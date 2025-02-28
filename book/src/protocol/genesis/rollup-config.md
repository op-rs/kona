# Rollup Configs

Rollup configurations are a consensus construct used to configure an Optimism Consensus client.
When an OP Stack chain is deployed into production or consensus nodes are configured to sync the chain,
certain consensus parameters can be configured. These parameters are defined in the
[OP Stack specs][spec-configurability].

Consensus parameters are consumed by OP Stack software through the `RollupConfig` type defined in the
[`maili-genesis`][genesis] crate.

## `RollupConfig` Type

The [`RollupConfig`][rc] type is defined in [`maili-genesis`][genesis].

Rollup configs can be loaded for a given chain id using [`maili-registry`][registry].
The `ROLLUP_CONFIG` mapping in the `maili-registry` provides a mapping from chain ids
to rollup config.

<!-- Links -->

{{#include ../../links.md}}
