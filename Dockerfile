# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
# See: https://hub.docker.com/layers/messense/rust-musl-cross/x86_64-musl/images/sha256-7ef452f6c731535a716e3f5a5d255fbe9720f35e992c9dee7d477e58542cfaf5?context=explore
FROM messense/rust-musl-cross@sha256:7ef452f6c731535a716e3f5a5d255fbe9720f35e992c9dee7d477e58542cfaf5 as builder
WORKDIR /app
COPY . /app
RUN rustup update && rustc --version
RUN cargo build --verbose --release
