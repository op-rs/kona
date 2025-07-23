# kona-node Chain ID Support

The `kona-node` CLI now supports flexible chain ID specification via the `--l2-chain-id` flag, powered by `alloy_chains::Chain`. This allows you to specify chain IDs using either:

## Numeric Chain IDs
```bash
# Optimism mainnet (chain ID 10)
kona-node --l2-chain-id 10 node [other args...]

# Ethereum mainnet (chain ID 1)  
kona-node --l2-chain-id 1 node [other args...]

# Base mainnet (chain ID 8453)
kona-node --l2-chain-id 8453 node [other args...]

# Any numeric chain ID
kona-node --l2-chain-id 999999 node [other args...]
```

## String Chain Names
```bash
# Optimism by name
kona-node --l2-chain-id optimism node [other args...]

# Ethereum mainnet by name
kona-node --l2-chain-id mainnet node [other args...]

# Base by name
kona-node --l2-chain-id base node [other args...]
```

## Short Flag
```bash
# Using short flag with numeric ID
kona-node -c 10 node [other args...]

# Using short flag with string name
kona-node -c optimism node [other args...]
```

## Environment Variable
```bash
# Set via environment variable
export KONA_NODE_L2_CHAIN_ID=optimism
kona-node node [other args...]

# Or with numeric ID
export KONA_NODE_L2_CHAIN_ID=10
kona-node node [other args...]
```

## Default Behavior
If no chain ID is specified, the default is chain ID 10 (Optimism mainnet).

## Error Handling
Invalid chain names will result in parsing errors:
```bash
# This will fail
kona-node --l2-chain-id invalid_chain node [other args...]
```

## Supported Chain Names
The supported chain names are those recognized by the `alloy_chains` crate, including:
- `mainnet` (Ethereum mainnet, chain ID 1)
- `optimism` (Optimism mainnet, chain ID 10) 
- `base` (Base mainnet, chain ID 8453)
- And many others...

Any unknown numeric chain ID will be accepted as-is for custom or test networks.