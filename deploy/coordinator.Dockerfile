# Coordinator and sim-worker. Build context: repository root.
#   docker build -f deploy/coordinator.Dockerfile .

FROM rust:1.95-slim AS build
WORKDIR /src
# The toolchain rust-toolchain.toml pins, in its own layer so code changes reuse it.
COPY rust-toolchain.toml .
RUN rustup toolchain install
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p coordinator -p sim-worker \
    && cp target/release/coordinator target/release/sim-worker /usr/local/bin/

FROM debian:trixie-slim
COPY --from=build /usr/local/bin/coordinator /usr/local/bin/sim-worker /usr/local/bin/
USER nobody
ENV COORDINATOR_ADDR=0.0.0.0:7700 RUST_LOG=info
EXPOSE 7700
CMD ["coordinator"]
