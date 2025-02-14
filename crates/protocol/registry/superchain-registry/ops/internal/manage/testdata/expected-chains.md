# Superchain Ecosystem

### sepolia

| Chain Name | OP Governed[^1] | Superchain Hardforks[^2] | Explorer | Public RPC | Sequencer RPC
|---|---|---|---|---|---|
| OP Sepolia Testnet | ✅ | ✅ | https://sepolia-optimistic.etherscan.io | `https://sepolia.optimism.io` | `https://sepolia-sequencer.optimism.io` |
| TestChain | ❌ | ✅ | https://foo.bar.net | `https://foo.bar.net` | `https://foo.bar.net` |


[^1]: Chains are governed by Optimism if their `L1ProxyAdminOwner` is set to the value specified by the standard config and [configurability.md](https://github.com/ethereum-optimism/specs/blob/main/specs/protocol/configurability.md#l1-proxyadmin-owner).
[^2]: Chains receive Superchain hardforks if they've specified a `superchain_time`. This means that they have opted-into Superchain-wide upgrades.
