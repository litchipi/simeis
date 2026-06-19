FROM rust:1.85.0-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --package simeis-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/simeis-server ./simeis-server

EXPOSE 8080

CMD ["./simeis-server"]
