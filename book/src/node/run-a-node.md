# Run a Node

> [!NOTE]
>
> This tutorial walks through running the `kona-node` as
> a binary. To use docker, head over to the
> [Docker Guide](./docker.md) which uses a `docker-compose`
> setup provided by `kona`. The `docker-compose` setup
> automatically bootstraps the `kona-node` with `op-reth`,
> provisioning grafana dashboards and a default Prometheus
> configuration. It is encouraged to follow the
> [Docker Guide](./docker.md) to avoid misconfigurations.

Kona aims to make operating a node easy. Below, we will walk
through building and running the `kona-node` binary.

Out of the box, the `kona-node` can be used for any `OP Stack`
chain that is part of the [`superchain-registry`][scr].

In order to use an out-of-band `OP Stack` chain, for example
a new devnet, you'll need to specify the rollup config using
the custom `--l2-config-file` cli flag. More on that in the
[More Detailed Node Docs Section](#-More-Detailed-Node-Docs).

For now, let's get started by building the binary.


### Building the Binary

Kona provides a variety of ways to build and consume the `kona-node`
binary or image.

- **Install the prebuilt binary** from the [op-rs/packages][packages]
  section on GitHub.
- **Run as a Docker image** or use it as a base image in your own
  Dockerfile (see the [docs][pdocs] for details).
- **Build from source** for maximum control and customization.

To build a Docker image locally, Kona provides a convenient
[docker buildx target][buildx].

```
just build-local kona-node
```

To build the binary from source, use Cargo.

```
cargo build --release --bin kona-node
```


### Running the Binary

`kona-node` is an L2 consensus client (also called a "rollup node").
This means that the node is a _consensus_-layer client, which needs
a corresponding execution-layer client in order to sync and follow
the L2 chain. A number of L2 execution layer clients are available,
since the OP Stack uses a minimal diff approach to L1 execution
clients, including [`op-reth`][op-reth] and [`op-geth`][op-geth].

This section will illustrate running the `kona-node` with an instance
of [`op-reth`][op-reth].

The `kona-node` requires a few CLI flags.

- `--l1-eth-rpc <L1_ETH_RPC>`
  URL of the L1 execution client RPC API.
- `--l1-beacon <L1_BEACON>`
  URL of the L1 beacon API.
- `--l2-engine-rpc <L2_ENGINE_RPC>`
  URL of the engine API endpoint of an L2 execution client.
- `--l2-provider-rpc <L2_PROVIDER_RPC>`
  An L2 RPC URL.

The L2 engine RPC and L2 provider RPC are both endpoints that
point to the execution layer client, [`op-reth`][op-reth].

An L1 beacon endpoint and rpc endpoint are also required to
fetch the L1 chain data that the L2 chain is derived from.

First, start an instance of [`op-reth`][op-reth]. The
[`op-reth` docs][op-reth-docs] provide very detailed instructions
for running `op-reth` nodes for OP Stack chains (L2). For this
demo, we'll use `base`, but any other OP Stack chain will do.

```
op-reth node \
    --chain base \
    --rollup.sequencer-http https://mainnet-sequencer.base.org \
    --http \
    --ws \
    --authrpc.port 9551 \
    --authrpc.jwtsecret /path/to/jwt.hex
```

Kona has a `generate-jwt` justfile target that can be used to
create the `jwt.hex` file. Run `just generate-jwt`.


> [!NOTE]
>
> The JWT token file path passed into `--authrpc.jwtsecret`
> **MUST** be the same as the one passed into the `kona-node`.
>
> This JWT token is how the `op-reth` client authenticates
> requests made by the `kona-node` to the engine rpc.
>
> By default, the `kona-node` will attempt to read a JWT token
> from a `jwt.hex` file in the local directory. If it cannot
> find one, it will create a JWT token in a new `jwt.hex` file.
>
> To specify the path to the file that contains the JWT token,
> pass the file path into the `--l2.jwt-secret` CLI flag or
> use the `KONA_NODE_L2_ENGINE_AUTH` environment variable.

Then, run the `kona-node` using Base's chain id - `8453`.

```
kona-node node \
    --l2-chain-id 8453 \
    --l1-eth-rpc <L1_RPC_URL> \
    --l1-beacon <L1_BEACON_URL> \
    --l2-engine-rpc http://127.0.0.1:9551 \
    --l2-provider-rpc http://127.0.0.1:8545
```

That's it! Your node should connect to P2P and start syncing
quickly.


#### Debugging

`kona-node` provides a `-v` (or `--v`) flag as a way to set
the "verbosity" level for logs. By default, the verbosity level
is set to `3`, which is the INFO level. The level ranges from `0`
being no logs to `5` which includes trace logs. Log levels are
listed below, with each level including the one below it.

- `5`: `TRACE` - very verbose logs that provides a detailed trace
- `4`: `DEBUG` - logs meant to print debugging information
- `3`: `INFO` - informational logs
- `2`: `WARN` - includes warning and error logs
- `1`: `ERROR` - only error logs are shown
- `0`: No logs

The default verbosity level is `-vvv` which is the `3` or `INFO`
level. To set the `kona-node` to print `DEBUG` logs or level `4`,
run the node like so: `kona-node node -vvvv`.

By default, the `kona-node` initializes its tracing (logging)
using the default environment variable filter [provided by the
tracing_subscriber crate][tracing-env]. This uses the value in
the `RUST_LOG` environment variable to set the tracing level
for specific targets. Effectively, `RUST_LOG` allows you to
bypass the default log level for the whole node or specific
log targets. For example, by prepending `RUST_LOG=engine=debug`
to the `kona-node` command (or setting that as an environment
variable), only `INFO` logs will be displayed except for the
`engine` log target which will also print `DEBUG` logs.

This comes in handy say for when we would like to debug Kona's
P2P stack, we could prepend `RUST_LOG=discv5=debug,libp2p=debug`
to view debug logs from only `discv5` and `libp2p` targets.


#### More Detailed Node Docs

There are a number of important defaults.

- The node will poll and update the runtime config every 10 minutes.
  Configurable via the `--l1.runtime-config-reload-interval` flag.
- The P2P stack is spun up. The libp2p swarm listens on TCP `9222`
  to receive block gossip. The `discv5` discovery service runs on
  UDP port `9223`. Peer scoring is enabled.
- An RPC server is exposed at `0.0.0.0:9545`. Websocket connections
  are disabled by default.
- Metrics are enabled, serving prometheus metrics on `0.0.0.0:9090`.
  This can be configured using the `--metrics.enabled`,
  `--metrics.port`, and `--metrics.addr` cli flags.

> [!NOTE]
>
> If a file path to a rollup config is _not_ specified via the
> `--l2-config-file` cli flag, the Rollup Config will be loaded
> via the [superchain registry][scr].
>
> A custom rollup config can either be specified through the
> `--l2-config-file` flag, or specific values may be overridden
> using a set of override flags provided by the `kona-node`.
>
> Override flags (for example `--canyon-override`) can be viewed
> in the help menu by running `kona-node node --help`. The only
> overrides currently supported are hardfork timestamps in seconds.

A set of CLI flags relating to the sequencer and supervisor are
also available to the `kona-node` binary.

Now, when the `kona-node` starts up, it should immediately spin up
the P2P stack. It will begin discovering valid peers on the network
with the same chain id (base - `8453`) and OP Stack enr key "opstack".
When valid peers are discovered, they are sent to the libp2p swarm
which attempts to connect to them and listen for block gossip.

Depending on the chain, and the P2P network topology, it may take
longer for the `kona-node` to establish a strong set of peers and
begin receiving block gossip. For larger, more mature chains like
OP Mainnet and Base, peer discovery should happen quickly via the
chain's P2P bootnodes.

Once the first unsafe L2 payload attributes (block) is received
from peers in the libp2p swarm, it is sent off to the `kona-node`'s
engine actor which will kick off execution layer sync on the
`op-reth` execution client. When this happens, the `op-reth` logs
will start to show that it is fetching past L2 blocks to sync the
chain to tip.

Once EL sync is finished, `kona-node`'s derivation actor will
kick off the derivation pipeline and begin deriving the L2 chain.
All the while, the P2P stack is separately receiving unsafe L2
blocks from the chain's sequencer, and sending them off to the
engine actor to insert into the chain.


### Configuring a Dockerfile

To learn more about running a `kona-node` using docker, check
out the [docker guide](./docker.md).


<!-- Hyperlinks -->

[tracing-env]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#method.from_default_env

[scr]: https://github.com/ethereum-optimism/superchain-registry/tree/main

[op-reth-docs]: https://reth.rs/run/opstack

[op-reth]: https://github.com/paradigmxyz/reth/blob/main/crates/optimism/bin/src/main.rs
[op-geth]: https://github.com/ethereum-optimism/op-geth

[buildx]: https://github.com/op-rs/kona/tree/main/docker
[packages]: https://github.com/orgs/op-rs/packages?repo_name=kona
[pdocs]: https://github.com/op-rs/kona/pkgs/container/kona%2Fkona-node/446969659?tag=latest
