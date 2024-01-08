# Create the build container to compile the hello world program
FROM rust as builder
WORKDIR /app
COPY . /app
RUN cargo check && cargo build --verbose --release --all