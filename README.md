# Navigate SensorHub

Async sensor acquisition framework for embedded systems.

# Navigate SensorHub

**Navigate SensorHub** is an async, modular, and feature-configurable sensor acquisition layer for Linux-based embedded platforms (e.g., Raspberry Pi, Jetson, Rockchip). It serves as a low-latency, containerizable sensor driver hub within the broader Navigate autonomy system.

## Key Features

- Async-per-sensor tasks using `tokio`, optimized for single- or multi-core embedded CPUs.
- Modular sensor driver architecture using a central `SensorFactory` registry.
- Bus abstraction layer with support for I²C (and planned SPI, CAN, Ethernet).
- TOML-based configuration for sensors and buses.
- Selective compilation using Cargo features (`--features="lsm6dsl lis3mdl"`).
- Clean, testable, and structured `SensorDataFrame` output for use in downstream fusion, AI, and control systems.
- Designed for compatibility with DO-178C / ISO 26262 coding principles.

## Example Usage

```bash
# Build with selected sensor features
cargo build --release --features="lsm6dsl lis3mdl"

# Run on device (e.g. Raspberry Pi)
sudo ./target/release/navigate_sensorhub
```

## Configuration

### config/buses.toml

```toml
[[bus]]
id = "i2c0"
type = "i2c"
path = "/dev/i2c-1"
```

### config/sensors.toml

```toml
[[sensor]]
id = "imu0"
driver = "lsm6dsl"
bus = "i2c0"
address = 0x6a
frequency = 100

[[sensor]]
id = "mag0"
driver = "lis3mdl"
bus = "i2c0"
address = 0x1c
frequency = 25
```

## Architecture

- Each sensor module implements `SensorDriver` and registers a `SensorFactory`.
- A central registry selects the correct driver at runtime from config.
- Sensor tasks independently push latest readings to shared memory or publisher channels.

## Next Steps

- Add more drivers (BMP388, BNO055, etc)
- Add ZMQ / Protobuf / shared memory publisher options
- Add system status/health reporting