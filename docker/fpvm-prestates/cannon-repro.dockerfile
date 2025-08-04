FROM ghcr.io/op-rs/kona/cannon-builder:0.3.0 AS client-build
SHELL ["/bin/bash", "-c"]

ARG CLIENT_BIN
