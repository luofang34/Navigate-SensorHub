/// Board-specific constants and configuration for BerryGPS-IMUv4
///
/// This board includes:
/// - ICM42688P 6-axis IMU
/// - LSM6DSL 6-axis IMU (alternative/backup)
/// - LIS3MDL magnetometer
/// - BMP388 barometer

/// Default configuration directory (embedded at compile time)
pub const CONFIG_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/config");

/// Embedded TOML configurations
pub const BUSES_TOML: &str = include_str!("../config/buses.toml");
pub const SENSORS_TOML: &str = include_str!("../config/sensors.toml");

/// Board name
pub const BOARD_NAME: &str = "BerryGPS-IMUv4";

/// Get the configuration path (allow override via CONFIG_PATH env var)
pub fn get_config_path() -> String {
    std::env::var("CONFIG_PATH").unwrap_or_else(|_| CONFIG_DIR.to_string())
}
