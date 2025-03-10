FROM ubuntu:22.04 AS host-build
SHELL ["/bin/bash", "-c"]

ARG CLIENT_TAG

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

# Build kona-host on the selected tag
RUN cd kona && \
  git checkout $CLIENT_TAG && \
  cargo build --workspace --bin kona-host --release && \
  mv ./target/release/kona-host /kona-host

FROM ubuntu:22.04 AS export-stage

# Copy the kona-host binary
COPY --from=host-build /kona-host .
