#[cfg(feature = "lsm6dsl")]
pub mod lsm6dsl;
#[cfg(feature = "lis3mdl")]
pub mod lis3mdl;

use async_trait::async_trait;
use crate::bus::i2c::I2CBus;


#[derive(Debug, Default, Clone, Copy)]
pub struct SensorDataFrame {
    pub accel: Option<[f32; 3]>,
    pub gyro: Option<[f32; 3]>,
    pub mag: Option<[f32; 3]>,
    pub temp: Option<f32>,
}

#[async_trait]
pub trait SensorDriver: Send + Sync {
    async fn init(&mut self, _bus: &mut I2CBus) -> Result<(), String>;
    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String>;
    fn id(&self) -> &str;
    fn bus(&self) -> &str;
}

pub trait SensorFactory: Sync {
    fn name(&self) -> &'static str;
    fn create(&self, id: String, address: u8, bus_id: String) -> Box<dyn SensorDriver + Send>;
}

#[cfg(feature = "lsm6dsl")]
pub use self::lsm6dsl::LSM6DSL_FACTORY;
#[cfg(feature = "lis3mdl")]
pub use self::lis3mdl::LIS3MDL_FACTORY;

pub static SENSOR_FACTORIES: &[&dyn SensorFactory] = &[
    #[cfg(feature = "lsm6dsl")]
    &LSM6DSL_FACTORY,
    #[cfg(feature = "lis3mdl")]
    &LIS3MDL_FACTORY,
];

pub fn create_sensor_driver(
    driver: &str,
    id: String,
    address: u8,
    bus_id: String,
) -> Result<Box<dyn SensorDriver + Send>, String> {
    SENSOR_FACTORIES
        .iter()
        .find(|f| f.name() == driver)
        .map(|f| f.create(id, address, bus_id))
        .ok_or_else(|| format!("Unsupported driver '{}'", driver))
}