use crate::config::sensor_config::SensorConfig;
use crate::config::load_bus_config;
use crate::sensors::{lsm6dsl::Lsm6dsl, SensorDriver};
use crate::bus::i2c::I2CBus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn init_all(sensor_config: &SensorConfig) -> Result<(Vec<Box<dyn SensorDriver>>, HashMap<String, Arc<Mutex<I2CBus>>>), String> {
    let bus_cfg = load_bus_config("config/buses.toml").map_err(|e| e.to_string())?;

    let mut bus_map = HashMap::new();
    for b in bus_cfg.buses.iter() {
        if b.r#type == "i2c" {
            let bus = I2CBus::new(&b.path).map_err(|e| e.to_string())?;
            bus_map.insert(b.id.clone(), Arc::new(Mutex::new(bus)));
        }
    }

    let mut sensors: Vec<Box<dyn SensorDriver>> = Vec::new();
    for s in sensor_config.sensors.iter() {
        if s.driver == "lsm6dsl" {
            let mut sensor = Lsm6dsl::new(s.id.clone(), s.address);
            let bus_arc = bus_map.get(&s.bus).ok_or_else(|| format!("Bus '{}' not found for sensor '{}'", s.bus, s.id))?;
            let mut bus = bus_arc.lock().await;
            sensor.init(&mut *bus).await?;
            sensors.push(Box::new(sensor));
        }
    }

    Ok((sensors, bus_map))
}
