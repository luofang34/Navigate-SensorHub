use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::errors::{SensorError, SensorResult};
use async_trait::async_trait;

// Register addresses for the LIS3MDL
const WHO_AM_I: u8 = 0x0F;
const CTRL_REG1: u8 = 0x20;
const CTRL_REG2: u8 = 0x21;
const CTRL_REG3: u8 = 0x22;
const CTRL_REG4: u8 = 0x23;
const OUT_X_L: u8 = 0x28;

// Sensitivity for +/- 4 gauss full scale
const SENSITIVITY_4GAUSS: f32 = 0.00014; // Tesla per LSB

pub struct Lis3mdl {
    id: String,
    address: u8,
    bus_id: String,
}

impl Lis3mdl {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        Self {
            id,
            address,
            bus_id,
        }
    }
}

#[async_trait]
impl SensorDriver for Lis3mdl {
    async fn init(&mut self, bus: &mut I2CBus) -> SensorResult<()> {
        // Verify device identity
        let mut who_am_i_buf = [0u8; 1];
        bus.read_bytes(self.address, WHO_AM_I, &mut who_am_i_buf)
            .await?;

        if who_am_i_buf[0] != 0x3D {
            return Err(SensorError::WrongChipId {
                sensor: self.id.clone(),
                expected: 0x3D,
                actual: who_am_i_buf[0],
            });
        }

        // Configure magnetometer:
        // CTRL_REG1: Temp sensor disabled, medium-performance mode, 80 Hz ODR
        bus.write_byte(self.address, CTRL_REG1, 0b01011100)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure CTRL_REG1: {}", e),
            })?;
        // CTRL_REG2: +/- 4 gauss full scale
        bus.write_byte(self.address, CTRL_REG2, 0b00000000)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure CTRL_REG2: {}", e),
            })?;
        // CTRL_REG3: Continuous-conversion mode
        bus.write_byte(self.address, CTRL_REG3, 0b00000000)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure CTRL_REG3: {}", e),
            })?;
        // CTRL_REG4: Z-axis medium-performance mode
        bus.write_byte(self.address, CTRL_REG4, 0b00000100)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure CTRL_REG4: {}", e),
            })?;

        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        let mut frame = SensorDataFrame::default();

        // Read magnetometer data
        let mut mag_buf = [0u8; 6];
        bus.read_bytes(self.address, OUT_X_L, &mut mag_buf)
            .await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read magnetometer data: {}", e),
            })?;

        let mag_raw = [
            i16::from_le_bytes([mag_buf[0], mag_buf[1]]),
            i16::from_le_bytes([mag_buf[2], mag_buf[3]]),
            i16::from_le_bytes([mag_buf[4], mag_buf[5]]),
        ];

        frame.mag = Some([
            mag_raw[0] as f32 * SENSITIVITY_4GAUSS,
            mag_raw[1] as f32 * SENSITIVITY_4GAUSS,
            mag_raw[2] as f32 * SENSITIVITY_4GAUSS,
        ]);

        Ok(frame)
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
