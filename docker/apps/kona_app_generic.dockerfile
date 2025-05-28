ARG REPO_LOCATION

################################
#   Dependency Installation    #
#            Stage             #
################################
FROM ubuntu:22.04 AS dep-setup-stage
SHELL ["/bin/bash", "-c"]

# Install deps
RUN apt-get update && apt-get install -y \
  build-essential \
  git \
  curl \
  ca-certificates \
  libssl-dev \
  clang \
  pkg-config

# Install rust
ENV RUST_VERSION=1.85
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --default-toolchain ${RUST_VERSION} --component rust-src
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-chef

################################
#    Local Repo Setup Stage    #
################################
FROM dep-setup-stage AS app-local-setup-stage

# Copy in the local repository
COPY . /kona

################################
#   Remote Repo Setup Stage    #
################################
FROM dep-setup-stage AS app-remote-setup-stage
SHELL ["/bin/bash", "-c"]

ARG TAG
ARG REPOSITORY

# Clone kona at the specified tag
RUN git clone https://github.com/${REPOSITORY} && \
  cd kona && \
  git checkout "${TAG}"

################################
#       App Build Stage        #
################################
FROM app-${REPO_LOCATION}-setup-stage AS app-setup

FROM dep-setup-stage AS build-entrypoint
ARG BIN_TARGET
ARG BUILD_PROFILE

WORKDIR /app

FROM build-entrypoint AS planner
COPY --from=app-setup kona .
RUN cargo chef prepare --recipe-path recipe.json

FROM build-entrypoint AS builder 
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN RUSTFLAGS="-C target-cpu=native" cargo chef cook --bin "${BIN_TARGET}" --profile "${BUILD_PROFILE}" --recipe-path recipe.json

# Build application
COPY --from=app-setup kona .
# Build the application binary on the selected tag
RUN RUSTFLAGS="-C target-cpu=native" cargo build --bin "${BIN_TARGET}" --profile "${BUILD_PROFILE}"

# Export stage
FROM ubuntu:22.04 AS export-stage
SHELL ["/bin/bash", "-c"]

ARG BIN_TARGET
ARG BUILD_PROFILE

# Copy in the binary from the build image.
COPY --from=builder "app/target/${BUILD_PROFILE}/${BIN_TARGET}" "/usr/local/bin/${BIN_TARGET}"

# Copy in the entrypoint script.
COPY ./docker/apps/entrypoint.sh /entrypoint.sh

# Export the binary name to the environment.
ENV BIN_TARGET="${BIN_TARGET}"
ENTRYPOINT [ "/entrypoint.sh" ]
