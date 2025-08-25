# Rollup

Unified OP Stack rollup binary that integrates Kona services as an Execution Extension (ExEx).

## Usage

For example, this shows how to run `rollup` with the `kona-node` as an execution extension.

```bash
./rollup node --l1.eth http://localhost:8545 --l1.beacon http://localhost:5052 --chain 10
```

## Architecture

- **Custom CLI**: Extends kona-node arguments with reth compatibility
- **ExEx Integration**: Embeds kona-node as a reth Execution Extension
- **Buffered Provider**: Caches L2 chain state for efficient processing
- **Event Processing**: Handles chain commits, reorgs, and reverts

## Key Files

- `src/main.rs` - Entry point and CLI parsing
- `src/cli.rs` - Command-line interface
- `src/exex.rs` - Kona Node ExEx implementation

## Configuration

Use `--kona.*` prefixed flags for kona-specific options to avoid conflicts with reth.
