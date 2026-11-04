# ==============================================================================
# Multi-stage Dockerfile for nanit-proxy (Security-Hardened & Minimal)
# ==============================================================================

# Build Stage
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev build-base ca-certificates

WORKDIR /app

# Copy dependency manifests and build scripts
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto

# Copy application source code
COPY src ./src

# Build release binary (statically linked against musl)
RUN cargo build --release --locked

# Runtime Stage
FROM alpine:3.21

# Install runtime security certs & timezone database
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -g 10001 -S nanit \
    && adduser -u 10001 -S nanit -G nanit

WORKDIR /app

# Copy compiled binary from builder
COPY --from=builder /app/target/release/nanit-proxy /usr/local/bin/nanit-proxy

# Run as unprivileged non-root user
USER 10001:10001

# Default environment
ENV RUST_LOG="nanit_proxy=info,warn"

ENTRYPOINT ["nanit-proxy"]
CMD ["run"]
