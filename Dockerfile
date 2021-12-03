FROM rust:latest as builder

RUN cargo install cargo-build-deps

COPY ./Cargo.toml .

RUN cargo build-deps --release

COPY ./src src
RUN  cargo build --release

FROM debian:buster-slim

RUN export DEBIAN_FRONTEND=noninteractive && \
    apt-get update && \
    apt-get -y install --no-install-recommends libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ztcf /usr/local/bin

ENTRYPOINT ["./usr/local/bin/ztcf"]
