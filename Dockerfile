FROM rustlang/rust:nightly as builder

WORKDIR /app

RUN apt-get update
RUN apt-get install -y musl-tools

COPY ./ ./

RUN rustup target add x86_64-unknown-linux-musl --toolchain=nightly
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest

WORKDIR /root

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release .

CMD ["./bike_blockchain_backend"]