FROM ubuntu:latest

#ENV KONA_NODE_L1 

WORKDIR /

COPY kona-node/kona/target/release/kona-node /usr/local/bin

RUN mkdir -p /11155420

COPY kona-node/bootstores/sepolia.json /11155420/bootstore.json
COPY jwttoken/jwt.hex /


#ENV KONA_NODE_L1 ${KONA_NODE_L1}

ENTRYPOINT kona-node \
    --l2-chain-id 11155420 \
    --metrics.enabled \
    --metrics.port 9431 \
    node \
    --l1 ${KONA_NODE_L1} \
    --l1-beacon ${KONA_NODE_L1_BEACON} \
    --l2 ${KONA_NODE_L2} \
    --l2-engine-jwt-secret "/jwt.hex" \
    --p2p.listen.tcp 9190 \
    --p2p.listen.udp 9191 \
    --p2p.scoring light \
    --p2p.ban.peers \
    --rpc.port 5060 \
    --p2p.bootstore /


#CMD [ "--l2-chain-id", "11155420", \
#      "--metrics.enabled", \
#      "--metrics.port", "9431", \
#      "node", \
#      "--l1", ${KONA_NODE_L1}, \
#      "--l1-beacon=${KONA_NODE_L1}", \
#      "--l2=${KONA_NODE_L2}", \
#      "--l2-engine-jwt-secret", "/jwt.hex", \
#      "--p2p.listen.tcp", "9190", \
#      "--p2p.listen.udp", "9191", \
#      "--p2p.scoring", "light", \
#      "--p2p.ban.peers", \
#      "--rpc.port", "5060", \
#      "--p2p.bootstore", "/" ]
