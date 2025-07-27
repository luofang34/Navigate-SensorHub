use i2cdev::linux::{LinuxI2CDevice, LinuxI2CError};
use i2cdev::core::I2CDevice;

pub struct I2CBus {
    device: LinuxI2CDevice,
}

impl I2CBus {
    pub fn new(path: &str) -> Result<Self, LinuxI2CError> {
        let device = LinuxI2CDevice::new(path, 0)?;
        Ok(Self { device })
    }

    pub async fn read_bytes(&mut self, address: u8, reg: u8, buf: &mut [u8]) -> Result<(), LinuxI2CError> {
        self.device.set_slave_address(address as u16)?;
        self.device.smbus_write_byte(reg)?;
        self.device.read(buf)
    }

    pub async fn write_byte(&mut self, address: u8, reg: u8, byte: u8) -> Result<(), LinuxI2CError> {
        self.device.set_slave_address(address as u16)?;
        self.device.smbus_write_byte_data(reg, byte)
    }
}
