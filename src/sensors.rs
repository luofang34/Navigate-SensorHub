use async_trait::async_trait;
use crate::bus::i2c::I2CBus;
use crate::errors::{SensorError, SensorResult};

#[derive(Debug, Default, Clone)]
pub struct SensorDataFrame {
    pub accel: Option<[f32; 3]>,
    pub gyro: Option<[f32; 3]>,
    pub mag: Option<[f32; 3]>,
    pub temp: Option<f32>,
    pub pressure_static: Option<f32>,
    pub pressure_pitot: Option<f32>,
    /// Attitude quaternion (w, x, y, z) from ATTITUDE_QUATERNION
    pub quaternion: Option<[f32; 4]>,
    /// Body angular velocity (roll, pitch, yaw rates in rad/s)
    pub angular_velocity_body: Option<[f32; 3]>,
}

#[async_trait]
pub trait SensorDriver: Send + Sync {
    async fn init(&mut self, _bus: &mut I2CBus) -> SensorResult<()>;
    async fn read(&self, bus: &mut I2CBus) -> SensorResult<SensorDataFrame>;
    fn id(&self) -> &str;
    fn bus(&self) -> &str;

    /// Downcast to any for dynamic type checking (needed for MAVLink sensor setup)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

#[cfg(feature = "lsm6dsl")]
pub mod lsm6dsl;
#[cfg(feature = "lis3mdl")]
pub mod lis3mdl;
#[cfg(feature = "bmp388")]
pub mod bmp388;
#[cfg(feature = "icm42688p")]
pub mod icm42688p;
#[cfg(feature = "mavlink_sensors")]
pub mod mavlink;

pub fn create_sensor_driver(
    driver: &str,
    id: String,
    address: u8,
    bus_id: String,
) -> SensorResult<Box<dyn SensorDriver + Send>> {
    match driver {
        #[cfg(feature = "lsm6dsl")]
        "lsm6dsl" => Ok(Box::new(lsm6dsl::Lsm6dsl::new(id, address, bus_id))),
        #[cfg(feature = "lis3mdl")]
        "lis3mdl" => Ok(Box::new(lis3mdl::Lis3mdl::new(id, address, bus_id))),
        #[cfg(feature = "bmp388")]
        "bmp388" => Ok(Box::new(bmp388::Bmp388::new(id, address, bus_id))),
        #[cfg(feature = "icm42688p")]
        "icm42688p" => Ok(Box::new(icm42688p::Icm42688p::new(id, address, bus_id))),
        #[cfg(feature = "mavlink_sensors")]
        "mavlink_imu" => Ok(Box::new(mavlink::MavlinkSensor::new(
            id, bus_id, mavlink::MavlinkSensorType::Imu{instance: 0}
        ))),
        #[cfg(feature = "mavlink_sensors")]
        "mavlink_baro" => Ok(Box::new(mavlink::MavlinkSensor::new(
            id, bus_id, mavlink::MavlinkSensorType::Barometer
        ))),
        #[cfg(feature = "mavlink_sensors")]
        "mavlink_mag" => {
            // Magnetometer is not implemented yet - TODO
            Err(SensorError::UnsupportedDriver { driver: "mavlink_mag (not yet implemented)".to_string() })
        }
        #[cfg(feature = "mavlink_sensors")]
        "mavlink_attitude" => Ok(Box::new(mavlink::MavlinkSensor::new(
            id, bus_id, mavlink::MavlinkSensorType::Attitude
        ))),
        _ => Err(SensorError::UnsupportedDriver { driver: driver.to_string() }),
    }
}