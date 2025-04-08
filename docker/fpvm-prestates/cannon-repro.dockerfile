################################################################
#                  Build Cannon @ `CANNON_TAG`                 #
################################################################

FROM ubuntu:22.04 AS cannon-build
SHELL ["/bin/bash", "-c"]

ARG TARGETARCH
ARG CANNON_TAG

# Install deps
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates make

ENV GO_VERSION=1.22.7

# Fetch go manually, rather than using a Go base image, so we can copy the installation into the final stage
RUN curl -sL https://go.dev/dl/go$GO_VERSION.linux-$TARGETARCH.tar.gz -o go$GO_VERSION.linux-$TARGETARCH.tar.gz && \
  tar -C /usr/local/ -xzf go$GO_VERSION.linux-$TARGETARCH.tar.gz
ENV GOPATH=/go
ENV PATH=/usr/local/go/bin:$GOPATH/bin:$PATH

# Clone and build Cannon @ `CANNON_TAG`
RUN git clone https://github.com/ethereum-optimism/optimism && \
  cd optimism/cannon && \
  git checkout $CANNON_TAG && \
  make && \
  cp bin/cannon /cannon-bin

################################################################
#               Build kona-client @ `CLIENT_TAG`               #
################################################################

FROM ghcr.io/op-rs/kona/cannon-builder:0.1.0 AS client-build
SHELL ["/bin/bash", "-c"]

ARG CLIENT_TAG

# Install deps
RUN apt-get update && apt-get install -y --no-install-recommends git

# Clone kona at the specified tag
RUN git clone https://github.com/op-rs/kona

# Build kona-client on the selected tag
RUN cd kona && \
  git checkout $CLIENT_TAG && \
  cargo build -Zbuild-std=core,alloc -p kona-client --bin kona --locked --profile release-client-lto && \
  mv ./target/mips64-unknown-none/release-client-lto/kona /kona-client-elf

################################################################
#      Create `prestate.bin.gz` + `prestate-proof.json`        #
################################################################

FROM ubuntu:22.04 AS prestate-build
SHELL ["/bin/bash", "-c"]

# Set env
ENV CANNON_BIN_PATH="/cannon"
ENV CLIENT_BIN_PATH="/kona-client-elf"
ENV PRESTATE_OUT_PATH="/prestate.bin.gz"
ENV PROOF_OUT_PATH="/prestate-proof.json"

# Copy cannon binary
COPY --from=cannon-build /cannon-bin $CANNON_BIN_PATH

# Copy kona-client binary
COPY --from=client-build /kona-client-elf $CLIENT_BIN_PATH

# Create `prestate.bin.gz`
# TODO: update `--type` once cannon supports `DCLO` / `DCLZ`.
RUN $CANNON_BIN_PATH load-elf \
  --path=$CLIENT_BIN_PATH \
  --out=$PRESTATE_OUT_PATH \
  --type multithreaded64-3

# Create `prestate-proof.json`
RUN $CANNON_BIN_PATH run \
  --proof-at "=0" \
  --stop-at "=1" \
  --input $PRESTATE_OUT_PATH \
  --meta ./meta.json \
  --proof-fmt "./%d.json" \
  --output "" && \
  mv 0.json $PROOF_OUT_PATH

################################################################
#                       Export Artifacts                       #
################################################################

FROM scratch AS export-stage

COPY --from=prestate-build /cannon .
COPY --from=prestate-build /kona-client-elf .
COPY --from=prestate-build /prestate.bin.gz .
COPY --from=prestate-build /prestate-proof.json .
