# Migration to Embedded-HAL Architecture

## Status: ✅ Infrastructure Complete, 🚧 Driver Migration In Progress

### What's Done

#### 1. Workspace Structure ✅
```
Navigate-SensorHub/
├── Cargo.toml                     # Workspace root
├── navigate-sensorhub/            # Core library (platform-agnostic)
└── boards/berrygps-imuv4/         # Board-specific configuration
```

#### 2. Platform Abstraction Layer ✅
- `src/hal.rs` - Platform selection (Linux/bare-metal)
- Uses `linux-embedded-hal` for Linux
- Ready for bare-metal HAL implementations (STM32, RP2040, etc.)

#### 3. Feature-Based Compilation ✅
```toml
cargo install --features "icm426xx-driver,mavlink_sensors" navigate-sensorhub-berrygps-imuv4
```
- Only compiles selected drivers
- Smaller binaries
- Faster compilation

#### 4. Adapter Pattern ✅
- `src/sensors/adapters/` - Generic sensor adapters
- Bridge between embedded-hal drivers and SensorDriver trait
- Ready for plug-and-play driver integration

### Current Limitation: ICM426xx Driver

The `icm426xx` crate (v0.3.2) currently:
- ✅ Supports SPI interface
- ❌ I2C support mentioned but not fully implemented
- 🔬 Requires nightly Rust (uses `generic_const_exprs`)

**BerryGPS-IMUv4 uses I2C**, so we can't use icm426xx yet.

### Solution: Legacy Driver + Migration Path

**Current**: Use existing `icm42688p` driver (works on I2C)
**Future**: When icm426xx adds I2C support, simply:
1. Complete `src/sensors/adapters/icm426xx.rs`
2. Update `boards/berrygps-imuv4/config/sensors.toml`:
   ```toml
   driver = "icm426xx"  # Change from "icm42688p"
   ```
3. Done! No other code changes needed.

### Benefits Achieved

✅ **Workspace Architecture** - Clean separation of lib + boards
✅ **Compile-Time Driver Selection** - Feature flags
✅ **Platform-Agnostic Foundation** - embedded-hal ready
✅ **Board Configurations** - Easy to add PCB variants
✅ **Migration Path Clear** - Infrastructure ready for embedded-hal drivers

### Adding a New Board

```bash
cp -r boards/berrygps-imuv4 boards/my-board
# Edit boards/my-board/Cargo.toml
# Edit boards/my-board/config/*.toml
cargo build -p navigate-sensorhub-my-board
```

### Next Steps

1. **Wait for icm426xx I2C support** - or contribute to the crate
2. **Migrate other sensors** to embedded-hal when drivers available:
   - LSM6DSL → Find/write embedded-hal driver
   - LIS3MDL → Find/write embedded-hal driver
   - BMP388 → Find/write embedded-hal driver
3. **Add bare-metal board** (e.g., STM32, RP2040) to demonstrate portability

### Testing Current Build

```bash
# Build with nightly (required by icm426xx dependency)
cargo build --workspace --release

# Run BerryGPS-IMUv4 variant
cargo run -p navigate-sensorhub-berrygps-imuv4
```

### Installation

```bash
# Install board-specific binary
cargo install --path boards/berrygps-imuv4

# Binary name is consistent across boards
navigate-sensorhub
```

---

**The architecture is production-ready. Driver migration will happen incrementally as embedded-hal drivers mature.**
