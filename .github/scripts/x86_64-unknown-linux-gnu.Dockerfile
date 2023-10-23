FROM ghcr.io/cross-rs/x86_64-unknown-linux-gnu:main@sha256:bf0cd3027befe882feb5a2b4040dc6dbdcb799b25c5338342a03163cea43da1b

RUN set-eux; apt-get update && \
    apt-get install --assume-yes clang libclang-dev unzip wget libssl-dev && \
    wget https://github.com/protocolbuffers/protobuf/releases/download/v21.10/protoc-21.10-linux-x86_64.zip && \
    unzip protoc-21.10-linux-x86_64.zip -d /usr/local \
    wget http://security.ubuntu.com/ubuntu/pool/main/o/openssl/libssl1.1_1.1.0g-2ubuntu4_amd64.deb && \
    dpkg -i libssl1.1_1.1.0g-2ubuntu4_amd64.deb
