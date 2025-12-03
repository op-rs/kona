FROM ubuntu:latest

#ENV KONA_NODE_L1

WORKDIR /

COPY kona-node/kona/target/release/kona-node /usr/local/bin

RUN mkdir -p /11155420

COPY kona-node/bootstores/sepolia.json /11155420/bootstore.json
COPY jwttoken/jwt.hex /


#ENV KONA_NODE_L1 ${KONA_NODE_L1}

#ENTRYPOINT [ "kona-node" ]

# \
#    --l2-chain-id 11155420 \
#    --metrics.enabled \
#    --metrics.port 9002 \
#    node \
#    --l1 ${L1_PROVIDER_RPC} \
#    --l1-beacon ${L1_BEACON_API} \
#    --l2 ${KONA_NODE_L2}:${OP_RETH_ENGINE_PORT} \
#    --l2-engine-jwt-secret "/jwt.hex" \
#    --p2p.listen.tcp 9223 \
#    --p2p.listen.udp 9223 \
#    --p2p.scoring light \
#    --p2p.ban.peers \
#    --rpc.port 5060 \
#    --p2p.bootstore /


# CMD [ "--l2-chain-id", "11155420", \
#       "--metrics.enabled", \
#       "--metrics.port", "${KONA_NODE_METRICS_PORT}", \
#       "node", \
#       "--l1-eth-rpc=${L1_PROVIDER_RPC}", \
#       "--l1-beacon=${L1_BEACON_API}", \
#       "--l2=${KONA_NODE_L2}:${OP_RETH_ENGINE_PORT}", \
#       "--l2-engine-jwt-secret", "/jwt.hex", \
#       "--p2p.listen.tcp", "9190", \
#       "--p2p.listen.udp", "9191", \
#       "--p2p.scoring", "light", \
#       "--p2p.ban.peers", \
#       "--rpc.port", "5060", \
#       "--p2p.bootstore", "/" ]

ENV KONA_NODE_L1 ${KONA_NODE_L1}

CMD  kona-node --chain optimism-sepolia \
      --metrics.enabled \
      --metrics.port ${KONA_NODE_METRICS_PORT} \
      node \
      --l1 ${L1_PROVIDER_RPC} \
      --l1-beacon ${L1_BEACON_API} \
      --l2 ${KONA_NODE_L2}:${OP_RETH_ENGINE_PORT} \
      --l2-engine-jwt-secret /jwt.hex \
      --rpc.port 5060 \
      --p2p.listen.tcp 9223 \
      --p2p.listen.udp 9223 \
      --p2p.scoring light \
      --p2p.bootstore /db
