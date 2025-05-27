set positional-arguments
alias t := tests
alias la := lint-all
alias l := lint-native
alias lint := lint-native
alias f := fmt-native-fix
alias b := build-native
alias h := hack

# default recipe to display help information
default:
  @just --list

# Run all tests (excluding online tests)
tests: test test-docs

# Test for the native target with all features. By default, excludes online tests.
test *args="-E '!test(test_online)'":
  cargo nextest run --workspace --all-features {{args}}

# Run all online tests
test-online:
  just test "-E 'test(test_online)'"

# Runs the tests with llvm-cov
llvm-cov-tests:
  cargo llvm-cov nextest --locked --workspace --lcov \
    --output-path lcov.info --all-features \
    --exclude kona-node --exclude kona-p2p --exclude kona-sources \
    --ignore-run-fail --profile ci -E '!test(test_online)'

# Runs benchmarks
benches:
  cargo bench --no-run --workspace --features test-utils --exclude example-gossip --exclude example-discovery

# Lint the workspace for all available targets
lint-all: lint-native lint-cannon lint-asterisc lint-docs

# Runs `cargo hack check` against the workspace
hack:
  cargo hack check --feature-powerset --no-dev-deps

# Fixes the formatting of the workspace
fmt-native-fix:
  cargo +nightly fmt --all

# Check the formatting of the workspace
fmt-native-check:
  cargo +nightly fmt --all -- --check

# Lint the workspace
lint-native: fmt-native-check lint-docs
  cargo clippy --workspace --all-features --all-targets -- -D warnings

# Lint the workspace (mips arch). Currently, only the `kona-std-fpvm` crate is linted for the `cannon` target, as it is the only crate with architecture-specific code.
lint-cannon:
  docker run \
    --rm \
    -v `pwd`/:/workdir \
    -w="/workdir" \
    ghcr.io/op-rs/kona/cannon-builder:0.1.0 cargo clippy -p kona-std-fpvm --all-features -Zbuild-std=core,alloc -- -D warnings

# Lint the workspace (risc-v arch). Currently, only the `kona-std-fpvm` crate is linted for the `asterisc` target, as it is the only crate with architecture-specific code.
lint-asterisc:
  docker run \
    --rm \
    -v `pwd`/:/workdir \
    -w="/workdir" \
    ghcr.io/op-rs/kona/asterisc-builder:0.1.0 cargo clippy -p kona-std-fpvm --all-features -Zbuild-std=core,alloc -- -D warnings

# Lint the Rust documentation
lint-docs:
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items

# Test the Rust documentation
test-docs:
  cargo test --doc --workspace --locked

# Build for the native target
build-native *args='':
  cargo build --workspace $@

# Build target for the `kona-node` docker image specify a custom tag.
build-node-with-tag TAG:
  docker build \
    --progress plain \
    -f docker/apps/kona_app_generic.dockerfile \
    --build-arg "BIN_TARGET=kona-node" \
    --build-arg "TAG={{TAG}}" \
    -t kona-node:local \
    .

# Build target for the `kona-supervisor` docker image specify a custom tag.
build-supervisor-with-tag TAG:
  docker build \
    --progress plain \
    -f docker/apps/kona_app_generic.dockerfile \
    --build-arg "BIN_TARGET=kona-supervisor" \
    --build-arg "TAG={{TAG}}" \
    -t kona-supervisor:local \
    .

# Build target for the `kona-node` docker image. Uses the current remote commit tag.
build-node:
  just build-node-with-tag $(git rev-parse HEAD)

# Build `kona-client` for the `cannon` target.
build-cannon-client:
  docker run \
    --rm \
    -v `pwd`/:/workdir \
    -w="/workdir" \
    ghcr.io/op-rs/kona/cannon-builder:0.1.0 cargo build -Zbuild-std=core,alloc -p kona-client --bin kona --profile release-client-lto

# Build `kona-client` for the `asterisc` target.
build-asterisc-client:
  docker run \
    --rm \
    -v `pwd`/:/workdir \
    -w="/workdir" \
    ghcr.io/op-rs/kona/asterisc-builder:0.1.0 cargo build -Zbuild-std=core,alloc -p kona-client --bin kona --profile release-client-lto

# Check for unused dependencies in the crate graph.
check-udeps:
  cargo +nightly udeps --workspace --all-features --all-targets

# Updates the pinned version of the monorepo
update-monorepo:
  #!/bin/bash
  [ ! -d monorepo ] && git clone https://github.com/ethereum-optimism/monorepo
  cd monorepo && \
    git fetch origin && \
    git checkout develop && \
    git rev-parse HEAD > ../.config/monorepo

# Clones the Optimism monorepo and checks out the pinned commit
monorepo:
  #!/bin/bash
  ([ ! -d monorepo ] && git clone https://github.com/ethereum-optimism/monorepo)
  (cd monorepo && git checkout "$(cat ../.config/monorepo)")

# Remove the monorepo directory
clean-monorepo:
  rm -rf monorepo/

# Installs the optimism-package repository
optimism-package:
  #!/bin/bash
  OPTIMISM_PACKAGE_REPO=$(jq -r '.repository' .config/optimism_package.json)
  OPTIMISM_PACKAGE_BRANCH=$(jq -r '.branch' .config/optimism_package.json)
  ([ ! -d optimism-package ] && git clone ${OPTIMISM_PACKAGE_REPO} optimism-package)
  (cd optimism-package && git fetch origin && git checkout ${OPTIMISM_PACKAGE_BRANCH})

# Remove the optimism-package directory
clean-optimism-package:
  rm -rf optimism-package/

# Spins up kurtosis with the `kona-node` docker image
kurtosis-up:
  #!/bin/bash
  # Check if the optimism-package directory exists
  if [ ! -d optimism-package ]; then
    echo "optimism-package directory not found. Installing with `just optimism-package`."
    just optimism-package
  fi

  # Check if the kurtosis command is available
  if ! command -v kurtosis &> /dev/null; then
    echo "kurtosis command not found. Please install kurtosis."
    exit 1
  fi

  # Run the kurtosis test
  (cd optimism-package && kurtosis run . --args-file ../.config/kurtosis_network_params.yaml)

# Winds down kurtosis, cleaning up the network
kurtosis-down:
  kurtosis clean -a

# Run action tests for the single-chain client program on the native target
action-tests-single test_name='Test_ProgramAction' *args='':
  #!/bin/bash
  echo "Setting up monorepo"
  just monorepo

  echo "Building contract artifacts for the monorepo"
  (cd monorepo/packages/contracts-bedrock && forge build)

  echo "Building host program for the native target"
  just build-native --bin kona-host
  export KONA_HOST_PATH="{{justfile_directory()}}/target/debug/kona-host"

  # GitHub actions patch - do not print logs.
  # https://github.com/gotestyourself/gotestsum/blob/b4b13345fee56744d80016a20b760d3599c13504/testjson/format.go#L442-L444
  echo "Running action tests for the client program on the native target"
  cd monorepo/op-e2e/actions/proofs && \
    GITHUB_ACTIONS=false gotestsum --format=testname -- -run "{{test_name}}" {{args}} -count=1 ./...

# Run action tests for the interop client program on the native target
action-tests-interop test_name='TestInteropFaultProofs' *args='':
  #!/bin/bash
  echo "Setting up monorepo"
  just monorepo

  echo "Building contract artifacts for the monorepo"
  (cd monorepo/packages/contracts-bedrock && forge build)

  echo "Building host program for the native target"
  just build-native --bin kona-host
  export KONA_HOST_PATH="{{justfile_directory()}}/target/debug/kona-host"

  # GitHub actions patch - do not print logs.
  # https://github.com/gotestyourself/gotestsum/blob/b4b13345fee56744d80016a20b760d3599c13504/testjson/format.go#L442-L444
  echo "Running action tests for the client program on the native target"
  cd monorepo/op-e2e/actions/interop && \
    GITHUB_ACTIONS=false gotestsum --format=testname -- -run "{{test_name}}" {{args}} -count=1 ./...

# Updates the `superchain-registry` git submodule source
source-registry:
  @just --justfile ./crates/protocol/registry/Justfile source

# Generate file bindings for super-registry
bind-registry:
  @just --justfile ./crates/protocol/registry/Justfile bind
