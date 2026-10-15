# syntax=docker/dockerfile:1.7
FROM --platform=linux/amd64 rust:1.91-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    bzip2 ca-certificates clang cmake curl git libclang-dev libprotobuf-dev \
    libssl-dev libudev-dev pkg-config protobuf-compiler zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-build-sbf --version 4.0.0 --locked
WORKDIR /work
COPY . /work
RUN mkdir -p /opt/solution \
    && cargo build-sbf --arch v0 --manifest-path /work/solve/Cargo.toml --sbf-out-dir /opt/solution \
    && cargo build --release -p forged-config-client

FROM --platform=linux/amd64 debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 libudev1 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /work/target/release/forged-config-client /usr/local/bin/submit
COPY --from=builder /opt/solution/solve.so /opt/solve.so
ENTRYPOINT ["/usr/local/bin/submit"]

