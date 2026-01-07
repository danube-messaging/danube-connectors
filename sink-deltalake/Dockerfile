# Build stage - use bookworm variant to match runtime
FROM rust:1.91-bookworm as builder

# Install protobuf compiler (required for danube-core gRPC compilation)
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

# Copy workspace configuration (single source of truth)
COPY Cargo.toml Cargo.lock ./

# Copy only the dependencies we need to build
COPY danube-connect-core ./danube-connect-core
COPY connectors/sink-deltalake ./connectors/sink-deltalake

# Filter workspace members to only include what we copied
# This keeps [workspace.dependencies] intact (single source of truth)
# but removes references to missing connectors
RUN sed -i '/members = \[/,/\]/c\
members = [\
    "danube-connect-core",\
    "connectors/sink-deltalake",\
]' Cargo.toml

# Build the connector
WORKDIR /usr/src/app
RUN cargo build --package danube-sink-deltalake --release

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates and curl for healthchecks
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder \
    /usr/src/app/target/release/danube-sink-deltalake \
    /usr/local/bin/danube-sink-deltalake

# Create non-root user
RUN useradd -m -u 1000 danube && \
    chown -R danube:danube /usr/local/bin/danube-sink-deltalake

USER danube

# Set environment defaults
ENV RUST_LOG=info
ENV LOG_LEVEL=info

ENTRYPOINT ["danube-sink-deltalake"]
