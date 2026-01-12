FROM ubuntu:latest

RUN apt-get update -y && apt-get upgrade -y && apt install -y ca-certificates

COPY kona-node/kona/target/release/kona-node /usr/local/bin

RUN useradd -ms /bin/bash konauser
USER konauser

WORKDIR /home/konauser

RUN mkdir -p /home/konauser/11155420

COPY kona-node/bootstores/sepolia.json /home/konauser/11155420/bootstore.json
COPY jwttoken/jwt.hex /home/konauser/

ENTRYPOINT [ "kona-node" ]
