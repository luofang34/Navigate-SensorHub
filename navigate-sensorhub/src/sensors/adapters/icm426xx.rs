/// Adapter for icm426xx embedded-hal driver
///
/// **Note**: The icm426xx crate currently only supports SPI interface.
/// I2C support is mentioned but not fully implemented/tested.
/// This adapter is a placeholder demonstrating the architecture pattern.
/// Once icm426xx adds stable I2C support, this can be completed.
///
/// For now, use the legacy `icm42688p` driver which supports I2C.

use crate::bus::i2c::I2CBus;
use crate::errors::{SensorError, SensorResult};
use crate::sensors::{SensorDataFrame, SensorDriver};
use async_trait::async_trait;

#[cfg(feature = "linux-hal")]
use crate::hal::I2CDevice;

/// Wrapper for ICM426xx series IMU using embedded-hal driver
pub struct Icm426xxAdapter {
    id: String,
    address: u8,
    bus_id: String,
    #[cfg(feature = "linux-hal")]
    driver: Option<icm426xx::ICM42688<I2CDevice>>,
}

impl Icm426xxAdapter {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        Self {
            id,
            address,
            bus_id,
            #[cfg(feature = "linux-hal")]
            driver: None,
        }
    }
}

#[async_trait]
impl SensorDriver for Icm426xxAdapter {
    #[cfg(feature = "linux-hal")]
    async fn init(&mut self, bus: &mut I2CBus) -> SensorResult<()> {
        use embedded_hal::i2c::I2c;

        // Create embedded-hal I2C device
        let i2c_dev = I2CDevice::new(&format!("/dev/i2c-{}",
            self.bus_id.strip_prefix("i2c").unwrap_or("0")))
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to create I2C device: {}", e),
            })?;

        // Create ICM42688 driver
        // Note: icm426xx uses SPI or I2C with ExclusiveDevice pattern
        // We'll need to wrap our I2C device appropriately
        // For now, this is a placeholder showing the structure

        // TODO: Properly initialize the icm426xx driver
        // This requires understanding the exact API of icm426xx crate
        // and potentially using embedded-hal-bus for device sharing

        tracing::info!(
            "[{}] ICM426xx adapter initialized (placeholder)",
            self.id
        );

        Ok(())
    }

    #[cfg(not(feature = "linux-hal"))]
    async fn init(&mut self, _bus: &mut I2CBus) -> SensorResult<()> {
        Err(SensorError::InitError {
            sensor: self.id.clone(),
            reason: "ICM426xx requires linux-hal feature".to_string(),
        })
    }

    async fn read(&self, _bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        // TODO: Implement actual read from icm426xx driver
        // This will involve:
        // 1. Reading accel data from driver
        // 2. Reading gyro data from driver
        // 3. Reading temp data from driver
        // 4. Converting to our SensorDataFrame format

        // Placeholder for now
        Err(SensorError::ReadError {
            sensor: self.id.clone(),
            reason: "ICM426xx read not yet implemented".to_string(),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn bus(&self) -> &str {
        &self.bus_id
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
