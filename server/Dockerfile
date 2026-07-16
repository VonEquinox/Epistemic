# Multi-stage build for epistemic-api / epistemic-worker
FROM rust:1.85-bookworm AS builder
WORKDIR /app
ARG BIN=epistemic-api

COPY Cargo.toml Cargo.toml
COPY crates crates

RUN cargo build --release -p ${BIN}

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 poppler-utils \
    && rm -rf /var/lib/apt/lists/*
ARG BIN=epistemic-api
COPY --from=builder /app/target/release/${BIN} /usr/local/bin/app
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/app"]
