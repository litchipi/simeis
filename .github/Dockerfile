FROM rust:1.85.0-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --package simeis-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/simeis-server ./simeis-server

ENV RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8080/ping || exit 1

CMD ["./simeis-server"]
