use crate::config::sensor_config::SensorConfig;
use crate::config::load_bus_config;
use crate::sensors::create_sensor_driver;
use crate::sensors::SensorDriver;
use crate::bus::i2c::I2CBus;
use crate::errors::{RegistryError, RegistryResult, SensorError, ConfigError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn init_all(sensor_config: &SensorConfig) -> RegistryResult<(Vec<Box<dyn SensorDriver>>, HashMap<String, Arc<Mutex<I2CBus>>>)> {
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let bus_config_path = format!("{}/buses.toml", config_path);
    let bus_cfg = load_bus_config(&bus_config_path)
        .map_err(|e| RegistryError::BusInitError(ConfigError::LoadError {
            path: bus_config_path,
            source: e,
        }))?;

    let mut bus_map = HashMap::new();
    for b in bus_cfg.buses.iter() {
        if b.r#type == "i2c" {
            let bus = I2CBus::new(&b.path)
                .map_err(|e| RegistryError::DriverCreationError(SensorError::BusNotFound { 
                    bus: b.id.clone() 
                }))?;
            bus_map.insert(b.id.clone(), Arc::new(Mutex::new(bus)));
        }
    }

    let mut sensors: Vec<Box<dyn SensorDriver>> = Vec::new();
    println!("[registry] initializing {} sensors...", sensor_config.sensors.len());
    for s in sensor_config.sensors.iter() {
        let mut sensor = create_sensor_driver(&s.driver, s.id.clone(), s.address, s.bus.clone())
            .map_err(|e| RegistryError::DriverCreationError(e))?;
        println!("[registry] registering sensor: id={} driver={} bus={}", s.id, s.driver, s.bus);
        
        let bus_arc = bus_map.get(&s.bus)
            .ok_or_else(|| RegistryError::DriverCreationError(SensorError::BusNotFound { 
                bus: s.bus.clone() 
            }))?;
        let mut bus = bus_arc.lock().await;
        sensor.init(&mut *bus).await
            .map_err(|e| RegistryError::RegistrationError(e))?;
        sensors.push(sensor);
    }

    Ok((sensors, bus_map))
}
