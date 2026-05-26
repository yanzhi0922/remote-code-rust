# =============================================================================
# Multi-stage Dockerfile for remote-code-rust services
# =============================================================================
# Build target is selected via --build-arg BINARY=<control-plane|runner>
#
# Build examples:
#   docker build --build-arg BINARY=remote-code-control-plane -t remote-code-control-plane .
#   docker build --build-arg BINARY=remote-code-runner        -t remote-code-runner .
#
# Run examples:
#   docker run -p 8080:8080 -p 4433:4433/udp remote-code-control-plane
#   docker run -p 8081:8080 remote-code-runner
# =============================================================================

ARG BINARY=remote-code-control-plane

# ---------------------------------------------------------------------------
# Stage 1: Build
# ---------------------------------------------------------------------------
# rust-toolchain.toml pins the exact channel (1.93.1). We start from a slim
# Debian image and install Rust via rustup so the toolchain file is honoured.
FROM debian:bookworm-slim AS builder

ARG BINARY

# Install build dependencies (git for crate fetching, pkg-config + openssl for
# TLS, and ca-certificates for HTTPS fetching during build).
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        git \
        pkg-config \
        libssl-dev \
        ca-certificates \
        curl \
        build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install Rust via rustup (non-interactive, minimal profile).
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal --default-toolchain none && \
    . /root/.cargo/env
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/remote-code-rust

# Cache dependency builds by copying manifests first.
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Install the pinned toolchain specified in rust-toolchain.toml.
RUN rustup show

# Copy all crate Cargo.toml files (workspace member manifests) so that
# cargo can resolve the full dependency graph before copying source.
COPY apps/remote-code-control-plane/Cargo.toml ./apps/remote-code-control-plane/Cargo.toml
COPY apps/remote-code-runner/Cargo.toml        ./apps/remote-code-runner/Cargo.toml
COPY agents/claudecode/Cargo.toml              ./agents/claudecode/Cargo.toml
COPY apps/remote-code-gui/src-tauri/Cargo.toml ./apps/remote-code-gui/src-tauri/Cargo.toml

# Shared crates
COPY crates/shared/  ./crates/shared/

# Claude crates
COPY crates/claude/  ./crates/claude/

# Adapter crates
COPY crates/adapters/ ./crates/adapters/

# Codex crates (large subtree — manifests only first, then full source below)
COPY crates/codex/   ./crates/codex/

# Roo crates
COPY crates/roo/     ./crates/roo/

# Create dummy main.rs files so cargo can resolve the workspace without full source.
# This maximises Docker layer caching for dependencies.
RUN mkdir -p apps/remote-code-control-plane/src && \
    echo 'fn main() {}' > apps/remote-code-control-plane/src/main.rs && \
    mkdir -p apps/remote-code-runner/src && \
    echo 'fn main() {}' > apps/remote-code-runner/src/main.rs

# Build dependencies only (cached layer).
RUN cargo build --release --package "${BINARY}" 2>/dev/null || true

# Now copy the real source over the dummies.
COPY . .

# Touch main.rs so cargo sees a newer timestamp than the cached build.
RUN find . -name "main.rs" -exec touch {} +

# Full build — dependencies are cached, only the application code recompiles.
RUN cargo build --release --package "${BINARY}"

# Strip debug symbols for a smaller binary.
RUN cp "target/release/${BINARY}" "/usr/local/bin/service-binary" && \
    strip "/usr/local/bin/service-binary"

# ---------------------------------------------------------------------------
# Stage 2: Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

ARG BINARY

# Install runtime dependencies (curl is needed for the health check).
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user and group for the service.
RUN groupadd --system remotecode && \
    useradd --system --gid remotecode --create-home --home-dir /home/remotecode remotecode

# Create directories for config, data, and downloads.
RUN mkdir -p /etc/remote-code /var/lib/remote-code /opt/remote-code/downloads && \
    chown -R remotecode:remotecode /var/lib/remote-code /opt/remote-code/downloads

# Copy the stripped binary from the builder.
COPY --from=builder /usr/local/bin/service-binary /usr/local/bin/remote-code-service

# Copy default entrypoint script.
COPY --chmod=755 <<'EOF' /usr/local/bin/docker-entrypoint.sh
#!/bin/sh
set -e

# If the first argument is the binary name or a subcommand, exec it;
# otherwise let the user override completely (e.g. /bin/sh).
if [ "$1" = "serve" ] || [ "$1" = "doctor" ] || [ "$1" = "print-config" ]; then
    exec /usr/local/bin/remote-code-service "$@"
fi

# Default: run serve subcommand.
exec /usr/local/bin/remote-code-service serve
EOF

USER remotecode

# HTTP API port.
EXPOSE 8080/tcp
# QUIC transport port.
EXPOSE 4433/udp

# Default volumes for persistent data.
VOLUME ["/etc/remote-code", "/var/lib/remote-code"]

# Health check — hits the built-in health endpoint.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["serve"]
