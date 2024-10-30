# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
# See: https://hub.docker.com/layers/messense/rust-musl-cross/aarch64-musl/images/sha256-bfcd06fbe849dbd90a7d3aef1454b0d7ffde9b2a89478bc4242c8d1b3592d560?context=explore
FROM messense/rust-musl-cross@sha256:799ee1796a7a7ac42d6b7769959325f726c2a56a765e9233f859cac28cddb20f as builder
WORKDIR /app
COPY . /app
RUN cargo build --verbose --release
