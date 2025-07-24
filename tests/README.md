# E2e testing for the kona-node

This repository contains the e2e testing resources for the kona-node. The e2e testing is done using the `devstack` from the [Optimism monorepo](https://github.com/ethereum-optimism/optimism). For now, only deployments with kurtosis are supported. The `devstack` is a framework that provides bindings to optimism devnets to help writing e2e tests.

## Installation

To install the dependencies, install [`mise`](https://mise.jdx.dev/) run the following command:

```bash
mise install
```

The [`mise file`](./mise.toml) contains the dependencies and the tools used in the e2e testing. This aims to replicate the one used in the [`monorepo`](https://github.com/ethereum-optimism/optimism/blob/develop/mise.toml).

## Description

The interactions with this repository are done through the [`justfile`](./justfile) recipes.

To run the e2e tests, run the following command:

```bash
just test-e2e DEVNET_NAME
```

Where `DEVNET_NAME` is the name of the devnet to deploy. The devnets are defined in the devnets directory. The `DEVNET_NAME` is the name of the devnet file without the `.yaml` extension. For example, to run the `simple-kona` devnet, run the following command:

```bash
just test-e2e simple-kona
```

Note that the recipe will generate a `DEVNET_NAME.json` file in the `devnets/specs` directory. This file contains the specifications of the devnet that is tied to the kurtosis devnet that is deployed. This file is used as a network specification for the e2e tests.

Note that in this example (and the ones below), the devnet will be run with a local docker image that is build from the current version of the code remotely deployed. For example, if working on the branch `my-branch`, the image will be built from the latest commit hash of the `my-branch` branch *present on the remote kona-node repository*. You may also specify a specific commit tag to build from by passing the commit tag as an argument to the `just test-e2e` recipe. For example, to run the `simple-kona` devnet with a specific commit tag, run the following command:

```bash
just test-e2e simple-kona <commit_tag>
```

### Other recipes

- `just devnet DEVNET_NAME`: Deploys the devnet specified by `DEVNET_NAME`. The `DEVNET_NAME` is the name of the devnet file without the `.yaml` extension. For example, to deploy the `simple-kona` devnet, run the following command:

```bash
just devnet simple-kona
```

- `just cleanup-kurtosis`: Winds down kurtosis, cleaning up the network.

- `just build-devnet BINARY`: Builds the docker image for the specified binary (`"node"` or `"supervisor"`).

- `just update-node-devnet DEVNET`: Updates the devnet with the latest local changes. This is useful to rapidly iterate on the devnet without having to redeploy the whole kurtosis network.


## Using `op-devstack` for testing

Set the following environment variables:

- `DEVSTACK_ORCHESTRATOR=sysext`: Environment variable to tell `op-devstack` to use devnet descriptor based backend e.g. local kurtosis network.

- `DISABLE_OP_E2E_LEGACY=true`: Environment variable to tell `op-devstack` not to use the `op-e2e` tests that rely on e2e config and contracts-bedrock artifacts.

Then, you can run the tests using:

```bash
just build-devnet-and-test-e2e DEVNET BINARY
```

### Hacks/Notes

For the `op-devstack` to properly parse the nodes of the network (inside the `mixed_preset`) we're using the following hacks/methods:

- All the kona nodes should have the `kona` string in their names.
- All the op-node nodes should have the `optimism` string in their names.
- All the sequencer nodes should have the `sequencer` string in their names.

## Contributing

We welcome contributions to this repository.
