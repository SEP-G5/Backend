FROM rustlang/rust:nightly as builder

WORKDIR /app

COPY ./ ./

RUN cargo build --release

FROM alpine:latest

WORKDIR /root

COPY --from=builder /app/target .

CMD ["./target/release/bike_blockchain_backend"]