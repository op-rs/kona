#!/usr/bin/env bash

RUST_LOG=batch_queue=error,derivation=trace,engine=info,engine_builder=debug,runtime=debug kona-node \
    --l2-chain-id 11155420 \
    --metrics.enabled \
    --metrics.port 9431 \
    node \
    --l1 $KONA_NODE_L1\
    --l1-beacon $KONA_NODE_L1 \
    --l2 $KONA_NODE_L2 \
    --l2-engine-jwt-secret "/jwt.hex" \
    --p2p.listen.tcp 9190 \
    --p2p.listen.udp 9191 \
    --p2p.scoring light \
    --p2p.ban.peers \
    --rpc.port 5060 \
    --p2p.bootstore /
