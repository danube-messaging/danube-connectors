# Build stage
FROM rust:1.91 as builder

# Install protobuf compiler (required for danube-core gRPC compilation)
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy workspace configuration (single source of truth)
COPY Cargo.toml Cargo.lock ./

# Copy only the dependencies we need to build
COPY danube-connect-core ./danube-connect-core
COPY connectors/sink-qdrant ./connectors/sink-qdrant

# Filter workspace members to only include what we copied
# This keeps [workspace.dependencies] intact (single source of truth)
# but removes references to missing connectors
RUN sed -i '/members = \[/,/\]/c\
members = [\
    "danube-connect-core",\
    "connectors/sink-qdrant",\
]' Cargo.toml

# Build the connector
WORKDIR /usr/src/app
RUN cargo build --package danube-sink-qdrant --release

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates for HTTPS/TLS connections
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder \
    /usr/src/app/target/release/danube-sink-qdrant \
    /usr/local/bin/danube-sink-qdrant

# Create non-root user
RUN useradd -m -u 1000 danube && \
    chown -R danube:danube /usr/local/bin/danube-sink-qdrant

USER danube

# Set environment defaults
ENV RUST_LOG=info
ENV LOG_LEVEL=info

ENTRYPOINT ["danube-sink-qdrant"]
