use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::errors::{SensorError, SensorResult};
use async_trait::async_trait;

// Register addresses for the ICM42688P
const WHO_AM_I: u8 = 0x75;
const DEVICE_CONFIG: u8 = 0x11;
const PWR_MGMT0: u8 = 0x4E;
const GYRO_CONFIG0: u8 = 0x4F;
const ACCEL_CONFIG0: u8 = 0x50;
const TEMP_DATA1: u8 = 0x1D;
const TEMP_DATA0: u8 = 0x1E;
const ACCEL_DATA_X1: u8 = 0x1F;
const GYRO_DATA_X1: u8 = 0x25;
const REG_BANK_SEL: u8 = 0x76;

// Expected WHO_AM_I values
const WHOAMI_ICM42688P: u8 = 0x47;
const WHOAMI_ICM42688: u8 = 0x44;

// Sensitivity values
const ACCEL_SENSITIVITY_2G: f32 = 16384.0;  // LSB/g
const GYRO_SENSITIVITY_250DPS: f32 = 131.0;  // LSB/dps
const TEMP_SENSITIVITY: f32 = 132.48;  // LSB/°C
const TEMP_OFFSET: f32 = 25.0;  // °C

pub struct Icm42688p {
    id: String,
    address: u8,
    bus_id: String,
}

impl Icm42688p {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        Self { id, address, bus_id }
    }
}

#[async_trait]
impl SensorDriver for Icm42688p {
    async fn init(&mut self, bus: &mut I2CBus) -> SensorResult<()> {
        // Select Bank 0
        bus.write_byte(self.address, REG_BANK_SEL, 0x00).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to select bank 0: {}", e),
            })?;

        // Verify device identity
        let mut who_am_i_buf = [0u8; 1];
        bus.read_bytes(self.address, WHO_AM_I, &mut who_am_i_buf).await?;
        
        if who_am_i_buf[0] != WHOAMI_ICM42688P && who_am_i_buf[0] != WHOAMI_ICM42688 {
            return Err(SensorError::WrongChipId {
                sensor: self.id.clone(),
                expected: WHOAMI_ICM42688P,
                actual: who_am_i_buf[0],
            });
        }

        // Soft reset
        bus.write_byte(self.address, DEVICE_CONFIG, 0x01).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to reset device: {}", e),
            })?;
        
        // Wait for reset to complete (15ms per datasheet)
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Configure power management - enable gyro and accel in low noise mode
        // Bits 3-2: Gyro mode = 11 (Low Noise)
        // Bits 1-0: Accel mode = 11 (Low Noise)
        bus.write_byte(self.address, PWR_MGMT0, 0x0F).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure power management: {}", e),
            })?;

        // Configure gyroscope: ±250 dps, 100 Hz ODR
        // Bits 7-5: FS_SEL = 011 (±250 dps)
        // Bits 3-0: ODR = 1000 (100 Hz)
        bus.write_byte(self.address, GYRO_CONFIG0, 0x68).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure gyroscope: {}", e),
            })?;

        // Configure accelerometer: ±2g, 100 Hz ODR
        // Bits 7-5: FS_SEL = 011 (±2g)
        // Bits 3-0: ODR = 1000 (100 Hz)
        bus.write_byte(self.address, ACCEL_CONFIG0, 0x68).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to configure accelerometer: {}", e),
            })?;

        // Wait for sensor stabilization
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        let mut frame = SensorDataFrame::default();

        // Read accelerometer data (6 bytes from ACCEL_DATA_X1)
        let mut accel_buf = [0u8; 6];
        bus.read_bytes(self.address, ACCEL_DATA_X1, &mut accel_buf).await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read accelerometer: {}", e),
            })?;
        
        // Data is big-endian in ICM42688P
        let accel_raw = [
            i16::from_be_bytes([accel_buf[0], accel_buf[1]]),
            i16::from_be_bytes([accel_buf[2], accel_buf[3]]),
            i16::from_be_bytes([accel_buf[4], accel_buf[5]]),
        ];
        
        // Convert to m/s^2
        frame.accel = Some([
            (accel_raw[0] as f32 / ACCEL_SENSITIVITY_2G) * 9.81,
            (accel_raw[1] as f32 / ACCEL_SENSITIVITY_2G) * 9.81,
            (accel_raw[2] as f32 / ACCEL_SENSITIVITY_2G) * 9.81,
        ]);

        // Read gyroscope data (6 bytes from GYRO_DATA_X1)
        let mut gyro_buf = [0u8; 6];
        bus.read_bytes(self.address, GYRO_DATA_X1, &mut gyro_buf).await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read gyroscope: {}", e),
            })?;
        
        // Data is big-endian in ICM42688P
        let gyro_raw = [
            i16::from_be_bytes([gyro_buf[0], gyro_buf[1]]),
            i16::from_be_bytes([gyro_buf[2], gyro_buf[3]]),
            i16::from_be_bytes([gyro_buf[4], gyro_buf[5]]),
        ];
        
        // Convert to degrees per second
        frame.gyro = Some([
            gyro_raw[0] as f32 / GYRO_SENSITIVITY_250DPS,
            gyro_raw[1] as f32 / GYRO_SENSITIVITY_250DPS,
            gyro_raw[2] as f32 / GYRO_SENSITIVITY_250DPS,
        ]);

        // Read temperature data (2 bytes)
        let mut temp_buf = [0u8; 2];
        bus.read_bytes(self.address, TEMP_DATA1, &mut temp_buf).await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read temperature: {}", e),
            })?;
        
        // Temperature data is big-endian
        let temp_raw = i16::from_be_bytes([temp_buf[0], temp_buf[1]]);
        
        // Convert to Celsius
        frame.temp = Some((temp_raw as f32 / TEMP_SENSITIVITY) + TEMP_OFFSET);

        Ok(frame)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn bus(&self) -> &str {
        &self.bus_id
    }
}