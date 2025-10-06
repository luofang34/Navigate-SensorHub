#[cfg(target_os = "linux")]
use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
#[cfg(target_os = "linux")]
use i2cdev::core::I2CDevice;

/// I2C bus error type - platform specific
#[cfg(target_os = "linux")]
pub type I2CError = LinuxI2CError;

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub struct I2CError(String);

#[cfg(not(target_os = "linux"))]
impl std::fmt::Display for I2CError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "I2C not supported on this platform: {}", self.0)
    }
}

#[cfg(not(target_os = "linux"))]
impl std::error::Error for I2CError {}

/// I2C bus implementation
#[cfg(target_os = "linux")]
pub struct I2CBus {
    device: LinuxI2CDevice,
}

#[cfg(not(target_os = "linux"))]
pub struct I2CBus {
    _phantom: std::marker::PhantomData<()>,
}

#[cfg(target_os = "linux")]
impl I2CBus {
    pub fn new(path: &str) -> Result<Self, I2CError> {
        let device = LinuxI2CDevice::new(path, 0)?;
        Ok(Self { device })
    }

    pub async fn read_bytes(&mut self, address: u8, reg: u8, buf: &mut [u8]) -> Result<(), I2CError> {
        self.device.set_slave_address(address as u16)?;

        if buf.len() == 1 {
            // Use SMBus read byte data for single byte reads
            let byte = self.device.smbus_read_byte_data(reg)?;
            buf[0] = byte;
        } else {
            // Use SMBus block read for multi-byte reads
            let temp_buf = self.device.smbus_read_i2c_block_data(reg, buf.len() as u8)?;
            buf.copy_from_slice(&temp_buf);
        }

        Ok(())
    }

    pub async fn write_byte(&mut self, address: u8, reg: u8, byte: u8) -> Result<(), I2CError> {
        self.device.set_slave_address(address as u16)?;
        self.device.smbus_write_byte_data(reg, byte)
    }
}

#[cfg(not(target_os = "linux"))]
impl I2CBus {
    pub fn new(_path: &str) -> Result<Self, I2CError> {
        Err(I2CError("I2C is only supported on Linux. For macOS, use MAVLink-only configuration.".to_string()))
    }

    pub async fn read_bytes(&mut self, _address: u8, _reg: u8, _buf: &mut [u8]) -> Result<(), I2CError> {
        Err(I2CError("I2C is only supported on Linux".to_string()))
    }

    pub async fn write_byte(&mut self, _address: u8, _reg: u8, _byte: u8) -> Result<(), I2CError> {
        Err(I2CError("I2C is only supported on Linux".to_string()))
    }
}
