# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
# See: https://hub.docker.com/layers/messense/rust-musl-cross/x86_64-musl/images/sha256-740c62dd2e08746df5fafa3fa47f5f2b0afb231c911e8b929c584d93c3baacae?context=explore
FROM messense/rust-musl-cross@sha256:47306f9557003a9cb1c63f5229c8276bc75e3bcf2be51e0d00f4abcc65fffaa4 as builder
WORKDIR /app
COPY . /app
RUN cargo build --verbose --release
