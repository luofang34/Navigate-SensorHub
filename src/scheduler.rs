use crate::sensors::SensorDriver;
use crate::bus::i2c::I2CBus;
use crate::config::sensor_config::SensorConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub async fn spawn_sensor_tasks(
    sensors: Vec<Box<dyn SensorDriver>>,
    buses: HashMap<String, Arc<Mutex<I2CBus>>>,
    sensor_config: &SensorConfig,
) {
    for sensor in sensors {
        let sensor_id = sensor.id().to_string();
        let config_entry = sensor_config.sensors.iter().find(|s| s.id == sensor_id).unwrap();
        let bus = buses.get(&config_entry.bus).unwrap().clone();
        let frequency = config_entry.frequency.unwrap_or(100); // Default to 100Hz
        let sleep_duration = Duration::from_millis((1000.0 / frequency as f32) as u64);

        tokio::spawn(async move {
            loop {
                let mut bus_lock = bus.lock().await;
                match sensor.read(&mut *bus_lock).await {
                    Ok(frame) => {
                        // In a real application, this would publish to ZMQ or another message bus.
                        println!("[{}] {:?}", sensor_id, frame);
                    }
                    Err(e) => {
                        eprintln!("[{}] error: {:?}", sensor_id, e);
                    }
                }
                sleep(sleep_duration).await;
            }
        });
    }
}
