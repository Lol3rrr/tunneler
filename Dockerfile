FROM --platform=$BUILDPLATFORM rust:1.56 as builder

RUN USER=root cargo new --bin tunneler
WORKDIR ./tunneler
COPY . ./
RUN cargo build --release
RUN rustc --print cfg

FROM --platform=$BUILDPLATFORM debian:buster-slim
ARG APP=/usr/src/app

RUN mkdir -p ${APP}

COPY --from=builder /tunneler/target/release/tunneler ${APP}/tunneler

WORKDIR ${APP}

ENTRYPOINT ["./tunneler"]
