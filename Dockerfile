FROM --platform=$BUILDPLATFORM rust:alpine AS chef
WORKDIR /app
ENV PKGCONFIG_SYSROOTDIR=/
RUN apk add --no-cache musl-dev openssl-dev zig perl make
RUN cargo install --locked cargo-zigbuild cargo-chef
RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

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

FROM scratch
WORKDIR /app
ARG TARGETPLATFORM
COPY --from=builder /app/${TARGETPLATFORM}/decay /usr/bin/decay

# Allow the server to bind and be available to the local network
ENV HOST="0.0.0.0"
CMD  ["/usr/bin/decay"]