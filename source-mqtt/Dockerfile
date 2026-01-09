# Build stage
FROM rust:1.91-bookworm as builder

# Install protobuf compiler (required for danube-core gRPC compilation)
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy only the dependencies we need to build
COPY source-mqtt ./source-mqtt

# Build the connector
WORKDIR /usr/src/app/source-mqtt
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Alternative: Match Danube's base (if needed for compatibility)
# FROM debian:bullseye-slim

# Install CA certificates for HTTPS/TLS connections
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder \
    /usr/src/app/source-mqtt/target/release/danube-source-mqtt \
    /usr/local/bin/danube-source-mqtt

# Create non-root user
RUN useradd -m -u 1000 danube && \
    chown -R danube:danube /usr/local/bin/danube-source-mqtt

USER danube

# Set environment defaults
ENV RUST_LOG=info
ENV LOG_LEVEL=info

ENTRYPOINT ["danube-source-mqtt"]
