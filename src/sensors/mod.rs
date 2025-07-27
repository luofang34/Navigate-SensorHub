#[cfg(feature = "lsm6dsl")]
pub mod lsm6dsl;

use async_trait::async_trait;
use crate::bus::i2c::I2CBus;

#[derive(Debug)]
pub struct SensorDataFrame {
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
    pub temp: f32,
}

#[async_trait]
pub trait SensorDriver: Send + Sync {
    async fn init(&mut self, _bus: &mut I2CBus) -> Result<(), String>;
    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String>;
    fn id(&self) -> &str;
}