# Build the Rust API server. SQLx checks queries against the metadata in
# .sqlx/ (regenerate with `cargo sqlx prepare -- --bin server`), so the build
# needs no database access.
FROM rust:1-bookworm AS builder
WORKDIR /app
ENV SQLX_OFFLINE=true

COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release --bin server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/server /app/server
ENV APP_ENV=production
CMD ["/app/server"]
