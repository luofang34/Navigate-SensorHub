#[cfg(feature = "linux-hal")]
pub mod i2c;

pub mod mavlink;
pub mod serial;

/// Bus type enum for different communication interfaces
#[derive(Debug, Clone)]
pub enum BusType {
    I2C,
    Serial,
}

impl BusType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "i2c" => Some(BusType::I2C),
            "serial" => Some(BusType::Serial),
            _ => None,
        }
    }
}
