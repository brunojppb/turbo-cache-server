# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
# See: https://hub.docker.com/layers/messense/rust-musl-cross/x86_64-musl/images/sha256-d3c1fbd71e737fe988bd7a141c171c8f4b5a7d072a0c84a58720fdf7cf4ded24?context=explore
FROM messense/rust-musl-cross@sha256:9bf63830ce63649fb54995c5fbbd36b993535208000909ad4f9993bf6e168154 as builder
WORKDIR /app
COPY . /app
RUN cargo build --verbose --release
