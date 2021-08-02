FROM rust:1-slim as base

ENV USER=root

WORKDIR /code
RUN cargo init
COPY Cargo.toml /code/Cargo.toml
COPY Cargo.lock /code/Cargo.lock
RUN cargo fetch

FROM base AS builder

COPY src /code/src
RUN cargo build --release

FROM rust:1-slim-buster

WORKDIR /app
COPY --from=builder /code/target/release/svg_render_server /app/svg_render_server

EXPOSE 8080

ENTRYPOINT ["/app/svg_render_server"]