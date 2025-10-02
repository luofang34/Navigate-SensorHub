use crate::config::sensor_config::SensorConfig;
use crate::config::load_bus_config;
use crate::sensors::create_sensor_driver;
use crate::sensors::SensorDriver;
use crate::bus::i2c::I2CBus;
use crate::bus::serial::SerialBus;
use crate::bus::mavlink::MavlinkConnection;
use crate::bus::BusType;
use crate::errors::{RegistryError, RegistryResult, SensorError, ConfigError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, error};

pub async fn init_all(sensor_config: &SensorConfig) -> RegistryResult<(Vec<Box<dyn SensorDriver>>, HashMap<String, Arc<Mutex<I2CBus>>>)> {
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let bus_config_path = format!("{}/buses.toml", config_path);
    let bus_cfg = load_bus_config(&bus_config_path)
        .map_err(|e| RegistryError::BusInitError(ConfigError::LoadError {
            path: bus_config_path.clone(),
            source: e,
        }))?;

    let mut i2c_bus_map = HashMap::new();
    let mut mavlink_connections: HashMap<String, Arc<MavlinkConnection>> = HashMap::new();

    // Initialize buses based on type
    for b in bus_cfg.buses.iter() {
        let bus_type = BusType::from_str(&b.r#type)
            .ok_or_else(|| RegistryError::BusInitError(ConfigError::ValidationError(
                format!("Unknown bus type: {}", b.r#type)
            )))?;

        match bus_type {
            BusType::I2C => {
                info!("[registry] Initializing I2C bus: {} at {}", b.id, b.path);
                let bus = I2CBus::new(&b.path)
                    .map_err(|e| RegistryError::DriverCreationError(SensorError::BusNotFound {
                        bus: b.id.clone()
                    }))?;
                i2c_bus_map.insert(b.id.clone(), Arc::new(Mutex::new(bus)));
                info!("[registry] I2C bus {} initialized successfully", b.id);
            }
            BusType::Serial => {
                info!("[registry] Initializing Serial/MAVLink bus: {} at {}", b.id, b.path);
                let serial = SerialBus::new(&b.path)
                    .map_err(|e| {
                        error!("[registry] Failed to open serial port {}: {}", b.path, e);
                        RegistryError::DriverCreationError(SensorError::SerialError(e.into()))
                    })?;
                let mavlink_conn = MavlinkConnection::new(serial);
                mavlink_connections.insert(b.id.clone(), Arc::new(mavlink_conn));
                info!("[registry] Serial/MAVLink bus {} initialized successfully", b.id);
            }
        }
    }

    let mut sensors: Vec<Box<dyn SensorDriver>> = Vec::new();
    info!("[registry] Initializing {} sensor(s)...", sensor_config.sensors.len());

    for s in sensor_config.sensors.iter() {
        debug!("[registry] Creating sensor driver: id={} type={} bus={} addr=0x{:02X}",
               s.id, s.driver, s.bus, s.address);
        let mut sensor = create_sensor_driver(&s.driver, s.id.clone(), s.address, s.bus.clone())
            .map_err(|e| {
                error!("[registry] Failed to create sensor {}: {:?}", s.id, e);
                RegistryError::DriverCreationError(e)
            })?;
        info!("[registry] Sensor {} created successfully", s.id);

        // Check if this is a MAVLink sensor
        let is_mavlink_sensor = s.driver.starts_with("mavlink_");

        if is_mavlink_sensor {
            // For MAVLink sensors, set the connection and initialize without I2C bus
            let mavlink_conn = mavlink_connections.get(&s.bus)
                .ok_or_else(|| RegistryError::DriverCreationError(SensorError::BusNotFound {
                    bus: s.bus.clone()
                }))?;

            // Use dynamic dispatch to set MAVLink connection
            #[cfg(feature = "mavlink_sensors")]
            {
                use crate::sensors::{mavlink_imu, mavlink_baro, mavlink_mag};

                if let Some(imu) = sensor.as_any_mut().downcast_mut::<mavlink_imu::MavlinkImu>() {
                    imu.set_mavlink_connection(mavlink_conn.clone());
                } else if let Some(baro) = sensor.as_any_mut().downcast_mut::<mavlink_baro::MavlinkBaro>() {
                    baro.set_mavlink_connection(mavlink_conn.clone());
                } else if let Some(mag) = sensor.as_any_mut().downcast_mut::<mavlink_mag::MavlinkMag>() {
                    mag.set_mavlink_connection(mavlink_conn.clone());
                }
            }

            // For MAVLink sensors, we need to call init() but it doesn't actually use the I2C bus
            // Pass any available I2C bus as a dummy parameter (it won't be accessed)
            if let Some(i2c_bus) = i2c_bus_map.values().next() {
                let mut bus = i2c_bus.lock().await;
                sensor.init(&mut *bus).await
                    .map_err(|e| RegistryError::RegistrationError(e))?;
            } else {
                return Err(RegistryError::BusInitError(ConfigError::ValidationError(
                    "MAVLink sensors require at least one I2C bus to be configured (as a dummy parameter)".to_string()
                )));
            }
        } else {
            // For I2C sensors, use the I2C bus
            let bus_arc = i2c_bus_map.get(&s.bus)
                .ok_or_else(|| RegistryError::DriverCreationError(SensorError::BusNotFound {
                    bus: s.bus.clone()
                }))?;
            let mut bus = bus_arc.lock().await;
            sensor.init(&mut *bus).await
                .map_err(|e| RegistryError::RegistrationError(e))?;
        }

        sensors.push(sensor);
    }

    Ok((sensors, i2c_bus_map))
}
