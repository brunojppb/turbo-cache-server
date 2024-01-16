FROM rust as builder
WORKDIR /app
COPY . /app
RUN cargo check && cargo build --verbose --release
