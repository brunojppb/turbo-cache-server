FROM messense/rust-musl-cross@sha256:7ef452f6c731535a716e3f5a5d255fbe9720f35e992c9dee7d477e58542cfaf5 AS builder
WORKDIR /app
COPY . /app
# See: https://github.com/rust-lang/rustup/issues/1167#issuecomment-367061388
RUN rm -frv ~/.rustup/toolchains/*

RUN rustup show \
  && rustup update \
  && rustup target add x86_64-unknown-linux-musl \
  && rustc --version

RUN cargo build --verbose --release

FROM debian:buster-slim@sha256:bb3dc79fddbca7e8903248ab916bb775c96ec61014b3d02b4f06043b604726dc AS runtime

WORKDIR "/app"
# Making sure the app does not run as root
# as in case it breaks out of the container, it can't do anything
RUN chown nobody /app

RUN apt-get update -y \
  && apt-get install -y --no-install-recommends openssl ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder --chown=nobody:root /app/target/x86_64-unknown-linux-musl/release/decay ./

USER nobody
ENV HOST="0.0.0.0"

ENTRYPOINT ["./decay"]