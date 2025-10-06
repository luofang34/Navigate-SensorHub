use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::errors::{SensorError, SensorResult};
use async_trait::async_trait;

// Register addresses for the LSM6DSL
const WHO_AM_I: u8 = 0x0F;
const CTRL1_XL: u8 = 0x10;
const CTRL2_G: u8 = 0x11;
const OUT_TEMP_L: u8 = 0x20;
const OUTX_L_G: u8 = 0x22;
const OUTX_L_XL: u8 = 0x28;

const ACCEL_SENSITIVITY_2G: f32 = 0.061 * 9.81 / 1000.0; // m/s^2 per LSB
const GYRO_SENSITIVITY_250DPS: f32 = 8.75 / 1000.0; // dps per LSB

pub struct Lsm6dsl {
    id: String,
    address: u8,
    bus_id: String,
}

impl Lsm6dsl {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        Self {
            id,
            address,
            bus_id,
        }
    }
}

#[async_trait]
impl SensorDriver for Lsm6dsl {
    async fn init(&mut self, bus: &mut I2CBus) -> SensorResult<()> {
        // Verify device identity
        let mut who_am_i_buf = [0u8; 1];
        bus.read_bytes(self.address, WHO_AM_I, &mut who_am_i_buf)
            .await?;

        if who_am_i_buf[0] != 0x6A {
            return Err(SensorError::WrongChipId {
                sensor: self.id.clone(),
                expected: 0x6A,
                actual: who_am_i_buf[0],
            });
        }

        // Configure accelerometer: 104 Hz, 2g
        bus.write_byte(self.address, CTRL1_XL, 0b01000000)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure accelerometer: {}", e),
            })?;

        // Configure gyroscope: 104 Hz, 250 dps
        bus.write_byte(self.address, CTRL2_G, 0b01000000)
            .await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure gyroscope: {}", e),
            })?;

        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        let mut frame = SensorDataFrame::default();

        // Read accelerometer data
        let mut accel_buf = [0u8; 6];
        bus.read_bytes(self.address, OUTX_L_XL, &mut accel_buf)
            .await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read accelerometer: {}", e),
            })?;
        let accel_raw = [
            i16::from_le_bytes([accel_buf[0], accel_buf[1]]),
            i16::from_le_bytes([accel_buf[2], accel_buf[3]]),
            i16::from_le_bytes([accel_buf[4], accel_buf[5]]),
        ];
        frame.accel = Some([
            accel_raw[0] as f32 * ACCEL_SENSITIVITY_2G,
            accel_raw[1] as f32 * ACCEL_SENSITIVITY_2G,
            accel_raw[2] as f32 * ACCEL_SENSITIVITY_2G,
        ]);

        // Read gyroscope data
        let mut gyro_buf = [0u8; 6];
        bus.read_bytes(self.address, OUTX_L_G, &mut gyro_buf)
            .await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read gyroscope: {}", e),
            })?;
        let gyro_raw = [
            i16::from_le_bytes([gyro_buf[0], gyro_buf[1]]),
            i16::from_le_bytes([gyro_buf[2], gyro_buf[3]]),
            i16::from_le_bytes([gyro_buf[4], gyro_buf[5]]),
        ];
        frame.gyro = Some([
            gyro_raw[0] as f32 * GYRO_SENSITIVITY_250DPS,
            gyro_raw[1] as f32 * GYRO_SENSITIVITY_250DPS,
            gyro_raw[2] as f32 * GYRO_SENSITIVITY_250DPS,
        ]);

        // Read temperature data
        let mut temp_buf = [0u8; 2];
        bus.read_bytes(self.address, OUT_TEMP_L, &mut temp_buf)
            .await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read temperature: {}", e),
            })?;
        let temp_raw = i16::from_le_bytes([temp_buf[0], temp_buf[1]]);
        frame.temp = Some((temp_raw as f32 / 256.0) + 25.0);

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
