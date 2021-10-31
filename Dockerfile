FROM rust:1.56 as builder

RUN USER=root cargo new --bin tunneler
WORKDIR ./tunneler
COPY . ./
RUN cargo build --release

FROM debian:buster-slim
ARG APP=/usr/src/app

RUN mkdir -p ${APP}

COPY --from=builder /tunneler/target/release/tunneler ${APP}/tunneler

WORKDIR ${APP}

ENTRYPOINT ["./tunneler"]
