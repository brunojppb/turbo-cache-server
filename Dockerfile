FROM messense/rust-musl-cross:x86_64-musl@sha256:de2f3518049a4fb5b613b12c0dadc8f1d5ea040146e0903d4918acbcc0536d85 AS builder
# To make Decay compatible with differnt linux distributions,
# let's cross-compile using musl so the binary is statically linked with the right dependencies
# See: https://users.rust-lang.org/t/unable-to-run-compiled-program/88441/5
# See: https://github.com/rust-cross/rust-musl-cross
# See: https://hub.docker.com/layers/messense/rust-musl-cross/x86_64-musl/images/sha256-7ef452f6c731535a716e3f5a5d255fbe9720f35e992c9dee7d477e58542cfaf5?context=explore

WORKDIR /app
COPY . /app
# See: https://github.com/rust-lang/rustup/issues/1167#issuecomment-367061388
RUN rm -frv ~/.rustup
RUN rustup show \
  && rustup update \
  && rustup default stable \
  && rustup target add x86_64-unknown-linux-musl \
  && rustc --version
RUN cargo build --verbose --release

FROM scratch

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/decay /usr/bin/decay

ENV HOST="0.0.0.0"

CMD  ["/usr/bin/decay"]
