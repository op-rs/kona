# Hardfork activation inheritance behavior

There is a mechanism whereby hardfork activation times are set _superchain-wide_ in the `superchain.toml` configuration file present for each superchain target, but then conditionally propagated to all chains in that superchain. This streamlines the process to update many chains' hardfork activations in synchrony or for chains to opt in to receive hardforks at standard times automatically.

For a chain to "receive" a particular default (superchain-wide) hardfork activation time, the following conditions must hold:
* It must have the `superchain_time` set. All superchain-wide hardfork activations will be inherited starting from this timestamp.
* It must not set a non-nil value for this activation time in its individual configuration file
* The default hardfork activation must be set in the superchain-wide configuration file
* The hardfork activation time must be equal to or after the `superchain_time`
* The hardfork activation time must be strictly after the genesis of the chain in question. If it is equal to or before, the chain should receive the "magic value" of `0` for this hardfork.

To "receive" a hardfork activation time for a given chain means that the downstream OP Stack components will apply that activation time when starting up using a network flag specifying the chain in question.

At the time of writing, this is implemented for
* [op-geth](https://docs.optimism.io/builders/node-operators/configuration/base-config#initialization-via-network-flags)
* [op-node](https://docs.optimism.io/builders/node-operators/configuration/base-config#configuring-op-node)

These components load configuration from the superchain registry. This implies some more conditions which need to hold for a chain to receive the superchain-wide hardfork activation:
* They must be running the above OP Stack software which supports this feature, with the relevant initialization invocations to trigger it
* The software must be up-to-date enough to embed the latest superchain-wide default hardfork activation times as well as the chain's individual configuration file (complete with `superchain_time` field).

> [!CAUTION]
> If (for example) OP Stack components are initialized without the network flags, this will require manual coordination to pass hardfork activation times into the command line invocation of the relevant commands.
