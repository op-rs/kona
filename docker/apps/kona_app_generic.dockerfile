FROM ubuntu:22.04 AS app-build
SHELL ["/bin/bash", "-c"]

ARG TAG
ARG BIN_TARGET

# Install deps
RUN apt-get update && apt-get install -y --no-install-recommends \
  build-essential \
  git \
  curl \
  ca-certificates \
  libssl-dev \
  clang \
  pkg-config

# Install rust
ENV RUST_VERSION=1.85.0
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --default-toolchain ${RUST_VERSION} --component rust-src
ENV PATH="/root/.cargo/bin:${PATH}"

# Clone kona at the specified tag
RUN git clone https://github.com/op-rs/kona

# Build the application binary on the selected tag
RUN cd kona && \
  git checkout "${TAG}" && \
  cargo build --workspace --bin "${BIN_TARGET}" --release && \
  mv "./target/release/${BIN_TARGET}" "/${BIN_TARGET}"

FROM ubuntu:22.04 AS export-stage
SHELL ["/bin/bash", "-c"]

ARG BIN_TARGET

# Copy in the binary from the build image.
COPY --from=app-build "${BIN_TARGET}" "/usr/local/bin/${BIN_TARGET}"

# Copy in the entrypoint script.
COPY ./docker/apps/entrypoint.sh /entrypoint.sh

# Export the binary name to the environment.
ENV BIN_TARGET="${BIN_TARGET}"
ENTRYPOINT [ "/entrypoint.sh" ]
