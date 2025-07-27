# Stage 1: Build
FROM rust:1.77-slim as builder
WORKDIR /app
ARG SENSOR_FEATURES="lsm6dsl"
RUN apt-get update && apt-get install -y libi2c-dev
COPY . .
RUN cargo build --release --features="${SENSOR_FEATURES}"

# Stage 2: Runtime
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y libi2c-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/navigate_sensorhub /usr/local/bin/navigate_sensorhub
ENTRYPOINT ["/usr/local/bin/navigate_sensorhub"]
