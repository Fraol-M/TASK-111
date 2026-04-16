# ---- Stage 1: Base (OS dependencies) ----
FROM rust:1.88-slim AS base

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ---- Stage 2: Builder ----
FROM base AS builder

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && touch src/lib.rs
RUN cargo build --release
RUN rm -f target/release/deps/venue_ops* target/release/deps/libvenue_booking*

# Copy full source and build
COPY . .
RUN cargo build --release

# ---- Stage 2: Runtime ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary and migrations
COPY --from=builder /app/target/release/venue_ops ./venue_ops
COPY --from=builder /app/migrations ./migrations

# Create storage directories
RUN mkdir -p storage/backups storage/reconciliation_imports storage/attachments

EXPOSE 8080

CMD ["./venue_ops"]
