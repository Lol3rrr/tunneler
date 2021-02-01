FROM rust:1.43 as builder

RUN USER=root cargo new --bin tunneler
WORKDIR ./tunneler
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs

ADD . ./

RUN rm ./target/release/deps/tunneler*
RUN cargo build --release

FROM debian:buster-slim
ARG APP=/usr/src/app

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=builder /tunneler/target/release/tunneler ${APP}/tunneler

RUN chown -R $APP_USER:$APP_USER ${APP}

WORKDIR ${APP}

ENTRYPOINT ["./tunneler"]
