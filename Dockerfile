# ==============================================================================
# Quentin Torrentino Dockerfile
# Multi-stage build: Node (dashboard) + Rust (server) -> minimal runtime
# ==============================================================================

# ------------------------------------------------------------------------------
# Stage 1: Build dashboard (Vue 3 + Vite)
# ------------------------------------------------------------------------------
FROM node:22-slim AS dashboard-builder

WORKDIR /app/dashboard

# Install dependencies first (better layer caching)
COPY crates/dashboard/package.json crates/dashboard/package-lock.json* ./
RUN npm ci --ignore-scripts

# Copy source and build
COPY crates/dashboard/ ./
RUN npm run build

# ------------------------------------------------------------------------------
# Stage 2: Build Rust server
# ------------------------------------------------------------------------------
FROM rust:1.91-bookworm AS server-builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a dummy project to cache dependencies
RUN cargo new --bin dummy
WORKDIR /app/dummy

# Copy workspace Cargo files for dependency caching
COPY Cargo.toml Cargo.lock /app/
COPY crates/core/Cargo.toml /app/crates/core/Cargo.toml
COPY crates/server/Cargo.toml /app/crates/server/Cargo.toml

# Create dummy lib.rs for core crate
RUN mkdir -p /app/crates/core/src && echo "pub fn dummy() {}" > /app/crates/core/src/lib.rs
RUN mkdir -p /app/crates/server/src && echo "fn main() {}" > /app/crates/server/src/main.rs

# Build dependencies only (this layer is cached)
WORKDIR /app
RUN cargo build --release --package torrentino-server 2>/dev/null || true

# Now copy actual source code
COPY crates/core/src /app/crates/core/src
COPY crates/server/src /app/crates/server/src

# Touch source files to invalidate cache and rebuild
RUN touch /app/crates/core/src/lib.rs /app/crates/server/src/main.rs

# Build the actual binary
RUN cargo build --release --package torrentino-server

# ------------------------------------------------------------------------------
# Stage 3: Runtime image
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash quentin

# Create directories
RUN mkdir -p /app/dashboard /data /downloads /config \
    && chown -R quentin:quentin /app /data /downloads /config

WORKDIR /app

# Copy binary from builder
COPY --from=server-builder /app/target/release/quentin /app/quentin

# Copy dashboard from builder
COPY --from=dashboard-builder /app/dashboard/dist /app/dashboard

# Copy example config
COPY config.example.toml /app/config.example.toml

# Set ownership
RUN chown -R quentin:quentin /app

# Switch to non-root user
USER quentin

# Environment variables
ENV RUST_LOG=info
ENV DASHBOARD_DIR=/app/dashboard
ENV QUENTIN_CONFIG=/config/config.toml

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

# Run the server
ENTRYPOINT ["/app/quentin"]
