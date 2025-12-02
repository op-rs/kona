FROM ghcr.io/paradigmxyz/op-reth:nightly AS reth

FROM ubuntu:latest

#COPY op-reth/reth/target/release/reth /usr/local/bin

COPY --from=reth /usr/local/bin/op-reth /usr/local/bin/op-reth

WORKDIR /

COPY jwttoken/jwt.hex /

ENTRYPOINT [ "op-reth" ]

# CMD [ "node", \
#     "--datadir", "/db", \
#     "--chain", "optimism-sepolia", \
#     "--metrics", "0.0.0.0:9430", \
#     "--rollup.sequencer-http", "https://sepolia-sequencer.optimism.io/", \
#     "--http", \
#     "--http.port", "8200", \
#     "--http.addr", "0.0.0.0", \
#     "--http.api", "debug,eth,net,trace,txpool,rpc,web3,admin", \
#     "--authrpc.jwtsecret", "jwt.hex", \
#     "--authrpc.addr", "0.0.0.0", \
#     "--authrpc.port", "8201", \
#     "--port", "30333", \
#     "--rpc.eth-proof-window", "4096", \
#     "-vvv" ]
