use async_trait::async_trait;
use crate::bus::i2c::I2CBus;

#[derive(Debug, Default, Clone)]
pub struct SensorDataFrame {
    pub accel: Option<[f32; 3]>,
    pub gyro: Option<[f32; 3]>,
    pub mag: Option<[f32; 3]>,
    pub temp: Option<f32>,
    pub pressure_static: Option<f32>,
    pub pressure_pitot: Option<f32>,
}

#[async_trait]
pub trait SensorDriver: Send + Sync {
    async fn init(&mut self, _bus: &mut I2CBus) -> Result<(), String>;
    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String>;
    fn id(&self) -> &str;
    fn bus(&self) -> &str;
}

#[cfg(feature = "lsm6dsl")]
pub mod lsm6dsl;
#[cfg(feature = "lis3mdl")]
pub mod lis3mdl;
#[cfg(feature = "bmp388")]
pub mod bmp388;

pub fn create_sensor_driver(
    driver: &str,
    id: String,
    address: u8,
    bus_id: String,
) -> Result<Box<dyn SensorDriver + Send>, String> {
    match driver {
        #[cfg(feature = "lsm6dsl")]
        "lsm6dsl" => Ok(Box::new(lsm6dsl::Lsm6dsl::new(id, address, bus_id))),
        #[cfg(feature = "lis3mdl")]
        "lis3mdl" => Ok(Box::new(lis3mdl::Lis3mdl::new(id, address, bus_id))),
        #[cfg(feature = "bmp388")]
        "bmp388" => Ok(Box::new(bmp388::Bmp388::new(id, address, bus_id))),
        _ => Err(format!("Unsupported driver '{}'", driver)),
    }
}