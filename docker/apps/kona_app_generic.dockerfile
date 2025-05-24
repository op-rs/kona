ARG REPO_LOCATION

################################
#   Dependency Installation    #
#            Stage             #
################################
FROM ubuntu:22.04 AS dep-setup-stage
SHELL ["/bin/bash", "-c"]

# Install deps
RUN apt-get update && apt-get install -y --no-install-recommends \
  build-essential \
  git \
  curl \
  ca-certificates \
  libssl-dev \
  clang \
  pkg-config

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
FROM app-${REPO_LOCATION}-setup-stage AS app-build-stage
SHELL ["/bin/bash", "-c"]

ARG BIN_TARGET
ARG BUILD_PROFILE

# Install rust
ENV RUST_VERSION=1.85.0
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --default-toolchain ${RUST_VERSION} --component rust-src
ENV PATH="/root/.cargo/bin:${PATH}"

# Build the application binary on the selected tag
RUN cd kona && \
  RUSTFLAGS="-C target-cpu=native" cargo build --workspace --bin "${BIN_TARGET}" --profile "${BUILD_PROFILE}" && \
  mv "./target/${BUILD_PROFILE}/${BIN_TARGET}" "/${BIN_TARGET}"

# Export stage
FROM ubuntu:22.04 AS export-stage
SHELL ["/bin/bash", "-c"]

ARG BIN_TARGET

# Copy in the binary from the build image.
COPY --from=app-build-stage "${BIN_TARGET}" "/usr/local/bin/${BIN_TARGET}"

# Copy in the entrypoint script.
COPY ./docker/apps/entrypoint.sh /entrypoint.sh

# Export the binary name to the environment.
ENV BIN_TARGET="${BIN_TARGET}"
ENTRYPOINT [ "/entrypoint.sh" ]
