# Rust toolchain version. This is the single fallback for standalone `docker build`.
# CI always overrides it with --build-arg RUST_VERSION read from rust-toolchain.toml,
# which is the single source of truth for the pinned version.
ARG RUST_VERSION=1.96.1

FROM alpine:3.24.3 AS ca-certificates
RUN apk add --no-cache ca-certificates

FROM --platform=$BUILDPLATFORM rust:alpine AS chef
ARG RUST_VERSION
WORKDIR /app
ENV PKGCONFIG_SYSROOTDIR=/
RUN apk add --no-cache musl-dev openssl-dev zig perl make && \
  rustup toolchain install ${RUST_VERSION} && rustup default ${RUST_VERSION} && \
  cargo install --locked cargo-zigbuild cargo-chef && \
  rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json \
  --release \
  --zigbuild \
  --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl

COPY . .
RUN cargo zigbuild -r \
  --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl && \
  mkdir /app/linux && \
  cp target/aarch64-unknown-linux-musl/release/decay /app/linux/arm64 && \
  cp target/x86_64-unknown-linux-musl/release/decay /app/linux/amd64

FROM ghcr.io/rust-cross/cargo-zigbuild AS macos-builder
ARG RUST_VERSION
# The image ships the macOS 11.3 SDK, which lacks IOKit symbols that sysinfo needs (macOS 12+).
ARG MACOS_SDK_VERSION=15.5
ARG MACOS_SDK_SHA256=c15cf0f3f17d714d1aa5a642da8e118db53d79429eb015771ba816aa7c6c1cbd
RUN curl -fsSL -o /tmp/sdk.tar.xz \
  "https://github.com/joseluisq/macosx-sdks/releases/download/${MACOS_SDK_VERSION}/MacOSX${MACOS_SDK_VERSION}.sdk.tar.xz" && \
  echo "${MACOS_SDK_SHA256}  /tmp/sdk.tar.xz" | sha256sum -c - && \
  tar -xJf /tmp/sdk.tar.xz -C /opt && \
  rm /tmp/sdk.tar.xz && \
  test -d "/opt/MacOSX${MACOS_SDK_VERSION}.sdk"
ENV SDKROOT=/opt/MacOSX${MACOS_SDK_VERSION}.sdk
WORKDIR /app
RUN rustup toolchain install ${RUST_VERSION} && rustup default ${RUST_VERSION} && \
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
COPY . .
RUN cargo zigbuild --release --target universal2-apple-darwin && \
  cp target/universal2-apple-darwin/release/decay /app/decay-darwin-universal

FROM scratch
WORKDIR /app
ARG TARGETPLATFORM
COPY --from=builder /app/${TARGETPLATFORM} /usr/bin/decay
COPY --from=ca-certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Allow the server to bind and be available to the local network
ENV HOST="0.0.0.0"
CMD  ["/usr/bin/decay"]
