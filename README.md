# Navigate SensorHub

**Navigate SensorHub** is an async, modular, and feature-configurable sensor acquisition layer for Linux-based embedded platforms (e.g., Raspberry Pi, Jetson, Rockchip). It provides high-performance sensor data streaming via gRPC for robotics and autonomous systems.

## Key Features

- **gRPC Streaming**: High-performance protobuf-based sensor data streaming with multi-subscriber support
- **Async Architecture**: Per-sensor tasks using `tokio`, optimized for single- or multi-core embedded CPUs
- **Modular Drivers**: Central `SensorFactory` registry with feature-based compilation
- **Bus Abstraction**: Support for I²C with planned SPI, CAN, Ethernet expansion
- **Docker Support**: Production-ready containerization for standalone and integrated deployments
- **TOML Configuration**: Simple, declarative sensor and bus configuration
- **Real-time Performance**: 80-100Hz sensor sampling with sub-millisecond latency

## Quick Start

### Native Installation

```bash
# Build with selected sensor features
cargo build --release --features="lsm6dsl lis3mdl bmp388"

# Run on device (requires I2C access)
sudo ./target/release/navigate_sensorhub

# Test sensor data streaming (in another terminal)
./test-grpc-client.sh
```

### Docker Deployment (Recommended)

```bash
# Start sensor hub
docker compose up -d

# View sensor data stream
docker run --rm --network host navigate-sensorhub-client

# Quick test - View IMU data
docker exec -it navigate-sensorhub-sensorhub-1 sh -c "grpcurl -plaintext localhost:50051 sensorhub.SensorHub/StreamIMU"

# Stop sensor hub
docker compose down
```

## Configuration

### config/sensors.toml

```toml
[[sensor]]
id = "imu0"
driver = "lsm6dsl"
bus = "i2c0"
address = 0x6a
frequency = 100  # Hz

[[sensor]]
id = "mag0"
driver = "lis3mdl"
bus = "i2c0"
address = 0x1c
frequency = 80

[[sensor]]
id = "static0"
driver = "bmp388"
bus = "i2c0"
address = 0x76
frequency = 80
```

### config/buses.toml

```toml
[[bus]]
id = "i2c0"
type = "i2c"
path = "/dev/i2c-1"
```

## gRPC API

### Service Definition

```protobuf
service SensorHub {
  rpc StreamIMU(SensorRequest) returns (stream IMUData);
  rpc StreamMagnetometer(SensorRequest) returns (stream MagnetometerData);
  rpc StreamBarometer(SensorRequest) returns (stream BarometerData);
  rpc StreamAll(SensorRequest) returns (stream SensorData);
}
```

### Client Example

```rust
// Connect to sensor hub
let mut client = SensorHubClient::connect("http://127.0.0.1:50051").await?;

// Stream IMU data
let stream = client.stream_imu(SensorRequest {}).await?;
let mut stream = stream.into_inner();

while let Some(data) = stream.message().await? {
    println!("IMU: accel=[{:.2}, {:.2}, {:.2}] m/s², gyro=[{:.2}, {:.2}, {:.2}] rad/s",
        data.accel_x, data.accel_y, data.accel_z,
        data.gyro_x, data.gyro_y, data.gyro_z);
}
```

## Docker Deployment

### Standalone Mode

The default `docker-compose.yml` provides everything needed for standalone operation:

```yaml
services:
  sensorhub:
    build:
      context: .
      args:
        FEATURES: "lsm6dsl lis3mdl bmp388"  # Select sensors
    ports:
      - "50051:50051"                        # gRPC port
    devices:
      - "/dev/i2c-1:/dev/i2c-1"             # I2C device
    volumes:
      - "./config:/app/config:ro"            # Configuration
    environment:
      - GRPC_HOST=0.0.0.0                   # Accept external connections
      - GRPC_PORT=50051
    restart: unless-stopped
```

### Navigate Project Integration

For integration with the larger Navigate system:

```yaml
# In Navigate's docker-compose.override.yml
services:
  sensorhub:
    build: ./Navigate-SensorHub
    networks:
      - navigate-network        # Internal network only
    devices:
      - "/dev/i2c-1:/dev/i2c-1"
    # No port exposure - internal access only
```

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `GRPC_HOST` | `127.0.0.1` | gRPC bind address (use 0.0.0.0 in Docker) |
| `GRPC_PORT` | `50051` | gRPC server port |
| `CONFIG_PATH` | `config` | Configuration directory path |

## Supported Sensors

| Sensor | Feature Flag | Type | Interface |
|--------|-------------|------|-----------|
| LSM6DSL | `lsm6dsl` | 6-DOF IMU | I²C |
| LIS3MDL | `lis3mdl` | Magnetometer | I²C |
| BMP388 | `bmp388` | Barometer | I²C |

Additional drivers can be added by implementing the `SensorDriver` trait.

## Architecture

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
│   gRPC Client   │────▶│  gRPC Server │◀────│   Sensors   │
└─────────────────┘     └──────────────┘     └─────────────┘
                               │                     ▲
                               ▼                     │
                        ┌──────────────┐      ┌──────────┐
                        │   Scheduler  │◀─────│   I²C    │
                        └──────────────┘      └──────────┘
```

- **Sensor Tasks**: Independent async tasks per sensor at configured frequencies
- **Message Channels**: Lock-free broadcast channels for multi-subscriber support  
- **gRPC Streaming**: Backpressure-aware streaming with automatic reconnection
- **Registry Pattern**: Dynamic sensor registration via factory pattern

## Testing

### Verify Sensor Communication

```bash
# Check I2C devices
sudo i2cdetect -y 1

# Test sensor hub locally
cargo test

# Run with debug output
RUST_LOG=debug cargo run
```

### Performance Monitoring

```bash
# Check Docker resource usage
docker stats navigate-sensorhub-sensorhub-1

# Monitor gRPC connections
netstat -an | grep 50051

# View container logs
docker compose logs -f
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "Failed to load sensor config" | Check config file exists and volume mount is correct |
| "Permission denied on /dev/i2c-1" | Add user to `i2c` group or run with `sudo` |
| "Transport error" connecting to gRPC | Ensure `GRPC_HOST=0.0.0.0` for Docker |
| "No sensor data received" | Verify I2C devices are connected and powered |

## Development

### Adding a New Sensor

1. Implement `SensorDriver` trait in `src/sensors/`
2. Register factory in sensor module
3. Add feature flag to `Cargo.toml`
4. Update protobuf schema if needed
5. Document in configuration examples

### Building from Source

```bash
# Install dependencies
sudo apt-get install protobuf-compiler pkg-config

# Build all features
cargo build --release --all-features

# Run tests
cargo test

# Generate docs
cargo doc --open
```

## License

Part of the Navigate autonomous systems project.

## Status