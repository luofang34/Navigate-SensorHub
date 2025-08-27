# Build stage with caching optimization
FROM rust:latest as builder
WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y protobuf-compiler pkg-config && rm -rf /var/lib/apt/lists/*

# Copy dependency files first for better layer caching
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ proto/

# Build dependencies first (cached layer if deps don't change)
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src/ src/

# Build with user-specified sensor features
ARG FEATURES="lsm6dsl lis3mdl bmp388"
RUN cargo build --release --features="${FEATURES}"

# Runtime stage - minimal
FROM debian:bookworm-slim
WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/navigate_sensorhub /usr/local/bin/navigate_sensorhub

# Copy default configs as examples
COPY config/ /app/config-examples/

# Create mount point for user configuration
RUN mkdir -p /app/config
VOLUME ["/app/config"]

# Expose gRPC port
EXPOSE 50051

# Set config path environment
ENV CONFIG_PATH=/app/config

ENTRYPOINT ["/usr/local/bin/navigate_sensorhub"]
