# Superchain Registry Contributing Guide

> [!WARNING]
> `CONTRIBUTING.md` contains guidelines for modifying or working with the code in the Superchain Registry, including the validation checks and exported modules.
>
> For guidelines on how to add a chain to the Registry, see the documentation for [adding a new chain](docs/ops.md#adding-a-chain).

The Superchain Registry repository contains:

- raw ["per-chain" config data](./README.md#3-understand-output) in `toml` and `json/json.gz` files arranged in a semantically meaningful directory structure
- [superchain-wide config data](#superchain-wide-config-data)
- a Go workspace with
  - a [`superchain`](#superchain-go-module) module
  - a [`validation`](#validation-go-module) module
  - an [`ops`](#ops-go-module) module
  - The modules are tracked by a top level `go.work` file. The associated `go.work.sum` file is gitignored and not important to typical workflows, which should mirror those of the [CI configuration](.circleci/config.yml).
- Automatically generated summary `chainList.json` and `chainList.toml` files.

## Superchain-wide config data

A superchain target defines a set of layer 2 chains which share a `SuperchainConfig` and `ProtocolVersions` contract deployment on layer 1. It is usually named after the layer 1 chain, possibly with an extra identifier to distinguish devnets.

> **Note**
> Example: `sepolia` and `sepolia-dev-0` are distinct superchain targets, although they are on the same layer 1 chain.

### Adding a superchain target

A new Superchain Target can be added by creating a new superchain config directory,
with a `superchain.yaml` config file.

> **Note**
> This is an infrequent operation and unnecessary if you are just looking to add a chain to an existing superchain.

Here's an example:

```bash
cd superchain-registry

export SUPERCHAIN_TARGET=goerli-dev-0
mkdir superchain/configs/$SUPERCHAIN_TARGET

cat > superchain/configs/$SUPERCHAIN_TARGET/superchain.yaml << EOF
name: Goerli Dev 0
l1:
  chain_id: 5
  public_rpc: https://ethereum-goerli-rpc.allthatnode.com
  explorer: https://goerli.etherscan.io

protocol_versions_addr: null # todo
superchain_config_addr: null # todo
EOF
```

Superchain-wide configuration, like the `ProtocolVersions` contract address, should be configured here when available.

## `superchain` Go Module

Per chain and superchain-wide configs and extra data are embedded into the `superchain` go module, which can be imported like so:

```shell
go get github.com/ethereum-optimism/superchain-registry/superchain@latest
```

The configs are consumed by downstream OP Stack software, i.e. `op-geth` and `op-node`.

## `validation` Go Module

A second module exists in this repo whose purpose is to validate the config exported by the `superchain` module. It is a separate module to avoid import cycles and polluting downstream dependencies with things like `go-ethereum` (which is used in the validation tests).

## `ops` Go module

This module contains the CLI tool for generating `superchain` compliant configs and extra data to the registry.

## CheckSecurityConfigs

The `security-configs_test.go` test is used in CI to perform
security checks of OP Chains registered in the `superchain`
directory. At high level, it performs checks to ensure privileges are
properly granted to the right addresses. More specifically, it checks
the following privilege grants and role designations:

1. Generic privileges:
   1. Proxy admins. For example, `L1ERC721BridgeProxy` and
      `OptimismMintableERC20FactoryProxy` specify the proxy admin
      addresses who can change their implementations.
   2. Address managers. For example, `ProxyAdmin` specifies the
      address manager it trusts to look up certain addresses by name.
   3. Contract owners. For example, many `Ownable` contracts use this
      role to specify the message senders allowed to make privileged
      calls.
2. Optimism privileged cross-contract calls:
   1. Trusted messengers. For example, `L1ERC721BridgeProxy` and
      `L1StandardBridgeProxy` specify the cross domain messenger
      address they trust with cross domain message sender information.
   2. Trusted bridges. For example,
      `OptimismMintableERC20FactoryProxy` specifies the L1 standard
      bridge it trusts to mint and burn tokens.
   3. Trusted portal. For example, `L1CrossDomainMessengerProxy`
      specifies the portal it trusts to deposit transactions and get
      L2 senders.
   4. Trusted oracles. For example, `OptimismPortalProxy` specifies
      the L2 oracle they trust with the L2 state root information.
      1. After the Fault Proofs upgrade, the `OptimismPortalProxy` specifies the `DisputeGameFactory` they trust rather
      than the legacy `L2OutputOracle` contract.
   5. Trusted system config. For example, `OptimismPortalProxy`
      specifies the system config they trust to get resource config
      from. TODO(issues/37): add checks for the `ResourceMetering`
      contract.
3. Optimism privileged operational roles:
   1. Guardians. This is the role that can pause withdrawals in the
      Optimism protocol.
      1. After the Fault Proofs upgrade, the `Guardian` can also blacklist dispute games and change the respected game type
         in the `OptimismPortal`.
   2. Challengers. This is the role that can delete `L2OutputOracleProxy`'s output roots in the Optimism protocol
      1. After the Fault Proofs upgrade, the `CHALLENGER` is a permissionless role in the `FaultDisputeGame`. However,
         in the `PermissionedDisputeGame`, the `CHALLENGER` role is the only party allowed to dispute output proposals
         created by the `PROPOSER` role.

As a result, here is a visualization of all the relationships the script checks:

``` mermaid
graph TD
  L1ERC721BridgeProxy -- "admin()" --> ProxyAdmin
  L1ERC721BridgeProxy -- "messenger()" --> L1CrossDomainMessengerProxy

  OptimismMintableERC20FactoryProxy -- "admin()" --> ProxyAdmin
  OptimismMintableERC20FactoryProxy -- "BRIDGE()" --> L1StandardBridgeProxy

  ProxyAdmin -- "addressManager()" --> AddressManager
  ProxyAdmin -- "owner()" --> ProxyOwnerMultisig

  L1CrossDomainMessengerProxy -- "PORTAL()" --> OptimismPortalProxy
  L1CrossDomainMessengerProxy -- "addressManager[address(this)]" --> AddressManager

  L1StandardBridgeProxy -- "getOwner()" -->  ProxyAdmin
  L1StandardBridgeProxy -- "messenger()" --> L1CrossDomainMessengerProxy

  AddressManager -- "owner()" -->  ProxyAdmin

  OptimismPortalProxy -- "admin()" --> ProxyAdmin
  OptimismPortalProxy -- "GUARDIAN()" --> GuardianMultisig
  OptimismPortalProxy -- "L2_ORACLE()" --> L2OutputOracleProxy
  OptimismPortalProxy -- "SYSTEM_CONFIG()" --> SystemConfigProxy
  OptimismPortalProxy -- "disputeGameFactory()" --> DisputeGameFactoryProxy

  L2OutputOracleProxy -- "admin()" --> ProxyAdmin
  L2OutputOracleProxy -- "CHALLENGER()" --> ChallengerMultisig

  SystemConfigProxy -- "admin()" --> ProxyAdmin
  SystemConfigProxy -- "owner()" --> SystemConfigOwnerMultisig

  DisputeGameFactoryProxy -- "admin()" --> ProxyAdmin
  DisputeGameFactoryProxy -- "owner()" --> ProxyAdminOwner

  AnchorStateRegistryProxy -- "admin()" --> ProxyAdmin

  DelayedWETHProxy -- "admin()" --> ProxyAdmin
  DelayedWETHProxy -- "owner()" --> ProxyAdminOwner
```

## Setting up your editor for formatting and linting

If you use VSCode, you can place the following in a `settings.json` file in the gitignored `.vscode` directory:

```json
{
    "go.formatTool": "gofumpt",
    "go.lintTool": "golangci-lint",
    "go.lintOnSave": "workspace",
    "gopls": {
        "formatting.gofumpt": true,
    },
}
```

## Links

See [Superchain Upgrades] OP Stack specifications.

[Superchain Upgrades]: https://specs.optimism.io/protocol/superchain-upgrades.html

## CircleCI Checks
The following CircleCI checks are not mandatory for submitting a pull request, but they should be reviewed:

- `ci/circleci: compute-genesis-diff`
- `ci/circleci: compute-rollup-config-diff`

These jobs will run at every commit in every branch.

For pull requests from forks, these checks will not appear directly in the PR comments, but **the jobs will still run** and their results can be viewed in the diffs.

Please note that while these jobs are **not blocking**, they must pass to ensure the accuracy of the changes.
