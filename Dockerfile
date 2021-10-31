# fetch the vendor with the builder platform to avoid qemu issues
FROM --platform=$BUILDPLATFORM rust:1.56-slim-buster AS sources

ENV USER=root

WORKDIR /code
RUN cargo init
COPY ./Cargo.toml /code/Cargo.toml
COPY ./Cargo.lock /code/Cargo.lock
RUN mkdir -p /code/.cargo \
  && cargo vendor > /code/.cargo/config

FROM rust:1.56 AS builder

ENV USER=root

WORKDIR /code

COPY ./Cargo.toml /code/Cargo.toml
COPY ./Cargo.lock /code/Cargo.lock
COPY ./src /code/src
COPY --from=server-sources /code/.cargo /code/.cargo
COPY --from=server-sources /code/vendor /code/vendor

RUN cargo build --release --offline

FROM --platform=$BUILDPLATFORM rust:1.56 as builder

WORKDIR ./tunneler
COPY . ./
RUN cargo build --release
RUN rustc --print cfg

FROM --platform=$BUILDPLATFORM debian:buster-slim
ARG APP=/usr/src/app

RUN mkdir -p ${APP}

COPY --from=builder /code/target/release/tunneler ${APP}/tunneler

WORKDIR ${APP}

ENTRYPOINT ["./tunneler"]
