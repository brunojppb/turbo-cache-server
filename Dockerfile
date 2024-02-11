# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
FROM messense/rust-musl-cross:x86_64-musl as builder
WORKDIR /app
COPY . /app
RUN cargo build --verbose --release
