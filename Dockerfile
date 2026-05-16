# ─────────────────────────────────────────────
# Stage 1: Builder
# ─────────────────────────────────────────────
FROM rust:1.78-bookworm AS builder

WORKDIR /app

# Cache dependencies first (layer caching trick)
COPY Cargo.toml Cargo.lock ./

# Create dummy main so cargo can fetch & compile deps
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null; rm -f target/release/flashcard-api

# Now copy real source and build
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ─────────────────────────────────────────────
# Stage 2: Runtime (minimal image)
# ─────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install runtime deps: ca-certs for TLS, libpq for postgres
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/flashcard-api ./flashcard-api

# Copy migrations (run at startup via init script)
COPY migrations ./migrations

EXPOSE 3000

CMD ["./flashcard-api"]
