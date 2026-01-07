# Build stage - use bookworm variant to match runtime
FROM rust:1.91-bookworm as builder

# Install protobuf compiler (required for danube-core gRPC compilation)
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy the sink-deltalake package
COPY sink-deltalake ./sink-deltalake

# Build the connector (standalone crate, not workspace)
WORKDIR /usr/src/app/sink-deltalake
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates and curl for healthchecks
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder \
    /usr/src/app/sink-deltalake/target/release/danube-sink-deltalake \
    /usr/local/bin/danube-sink-deltalake

# Create non-root user
RUN useradd -m -u 1000 danube && \
    chown -R danube:danube /usr/local/bin/danube-sink-deltalake

USER danube

# Set environment defaults
ENV RUST_LOG=info
ENV LOG_LEVEL=info

ENTRYPOINT ["danube-sink-deltalake"]
