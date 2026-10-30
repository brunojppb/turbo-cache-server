# CI overrides this from rust-toolchain.toml. Keep the standalone default in sync.
ARG RUST_VERSION=1.99.1
FROM rust:${RUST_VERSION}-alpine AS chef
WORKDIR /app
RUN apk add --no-cache musl-dev perl make && \
    cargo install --locked cargo-chef --version 0.1.78

FROM chef AS planner
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY tests ./tests
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY tests ./tests
RUN cargo build --release --locked --bin decay

# Export a binary directly; no image registry or temporary container needed.
FROM scratch AS artifact
COPY --from=builder /app/target/release/decay /decay

FROM alpine:3.24.3 AS certificates
RUN apk add --no-cache ca-certificates

# Standalone source build remains available with `docker build .`.
FROM scratch AS runtime
WORKDIR /app
COPY --from=artifact /decay /usr/bin/decay
COPY --from=certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENV HOST="0.0.0.0"
CMD ["/usr/bin/decay"]
