# Workaround for QEmu bug when building for 32bit platforms on a 64bit host
FROM --platform=$BUILDPLATFORM rust:latest as vendor
WORKDIR /app

COPY ./Cargo.toml Cargo.toml

RUN mkdir .cargo && cargo vendor > .cargo/config.toml

FROM rust:latest as builder
WORKDIR /app

COPY ./Cargo.toml .

COPY --from=vendor /app/.cargo .cargo
COPY --from=vendor /app/vendor vendor

# Without the workaround
# FROM rust:latest as builder

# RUN cargo install cargo-build-deps

# COPY ./Cargo.toml .
# COPY ./Cargo.lock .

# RUN cargo build-deps --release

COPY ./src src
RUN  cargo build --release

FROM debian:buster-slim

RUN export DEBIAN_FRONTEND=noninteractive && \
    apt-get update && \
    apt-get -y install --no-install-recommends libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ztcf /usr/local/bin

ENTRYPOINT ["./usr/local/bin/ztcf"]