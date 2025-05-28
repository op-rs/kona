# E2e testing for the kona-node

This repository contains the e2e testing resources for the kona-node. The e2e testing is done using the `devnet-sdk` from the [Optimism monorepo](https://github.com/ethereum-optimism/optimism). For now, only deployments with kurtosis are supported. The `devnet-sdk` is a framework that provides bindings to optimism devnets to help writing e2e tests.

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
just test DEVNET_NAME
```

Where `DEVNET_NAME` is the name of the devnet to deploy. The devnets are defined in the devnets directory. The `DEVNET_NAME` is the name of the devnet file without the `.yaml` extension. For example, to run the `simple-kona` devnet, run the following command:

```bash
just test simple-kona
```

Note that the recipe will generate a `DEVNET_NAME.json` file in the `devnets/specs` directory. This file contains the specifications of the devnet that is tied to the kurtosis devnet that is deployed. This file is used as a network specification for the e2e tests.

Note that in this example (and the ones below), the devnet will be run with a local docker image that is build from the current version of the code remotely deployed. For example, if working on the branch `my-branch`, the image will be built from the latest commit hash of the `my-branch` branch *present on the remote kona-node repository*. You may also specify a specific commit tag to build from by passing the commit tag as an argument to the `just test-e2e` recipe. For example, to run the `simple-kona` devnet with a specific commit tag, run the following command:

```bash
just test-e2e simple-kona <commit_tag>
```

### Other recipes

- `just deploy DEVNET_FILE_PATH`: Deploys the devnet specified by `DEVNET_FILE_PATH`. The `DEVNET_FILE_PATH` is the path to the devnet file in the `devnets` directory. For example, to deploy the `simple-kona` devnet, run the following command:

```bash
just deploy devnets/simple-kona.yaml
```

- `just isolate_test DEVNET_ENV_URL`: Runs the e2e tests for the devnet specified by `DEVNET_ENV_URL` - does not try to deploy a kurtosis network associated with the devnet. The `DEVNET_ENV_URL` is the URL of the devnet specification file in the `devnets/specs` directory. For example, to run the e2e tests for the `simple-kona` devnet, run the following command:

```bash
just isolate_test devnets/specs/simple-kona-devnet.json
```

- `just clone-kurtosis-devnet`: Clones the [optimism monorepo](https://github.com/ethereum-optimism/optimism) repository and installs the dependencies.

## Contributing

We welcome contributions to this repository.
