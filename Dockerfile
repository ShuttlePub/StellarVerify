FROM rust:1.84.1 AS build-stage

RUN mkdir /stellar
WORKDIR /stellar

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./migrations ./migrations
COPY ./application ./application
COPY ./driver ./driver
COPY ./kernel ./kernel
COPY ./server ./server

RUN cargo build --release

FROM gcr.io/distroless/cc

COPY --from=build-stage /stellar/target/release/server /

CMD ["/server"]