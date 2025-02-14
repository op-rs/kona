# Loading a Rollup Config from a Chain ID

In this section, the code examples demonstrate loading the
rollup config for the given L2 Chain ID.

Let's load the Rollup Config for OP Mainnet which hash chain id 10.

```rust
use maili_registry::ROLLUP_CONFIGS;
use maili_genesis::OP_MAINNET_CHAIN_ID;

// Load a rollup config from the chain id.
let op_mainnet_config = ROLLUP_CONFIGS.get(&OP_MAINNET_CHAIN_ID).expect("infallible");

// The chain id should match the hardcoded chain id.
assert_eq!(op_mainnet_config.chain_id, OP_MAINNET_CHAIN_ID);
```

> Available Configs
>
> [maili-registry][maili-registry] dynamically provides all rollup configs
> from the [superchain-registry][registry] for their respective chain ids.
> Note though, that this requires `serde` since it deserializes the rollup
> configs dynamically from json files.

[maili-registry]: https://crates.io/crates/maili-registry
[registry]: https://github.com/ethereum-optimism/superchain-registry
