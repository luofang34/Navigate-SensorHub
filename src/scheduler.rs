use crate::sensors::SensorDriver;
use crate::bus::i2c::I2CBus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub async fn spawn_sensor_tasks(
    sensors: Vec<Box<dyn SensorDriver>>,
    buses: HashMap<String, Arc<Mutex<I2CBus>>>,
) {
    for mut sensor in sensors.into_iter() {
        let sensor_id = sensor.id().to_string();
        let bus = buses.get(sensor.bus()).unwrap().clone();
        let frequency = 100; // Default to 100Hz
        let sleep_duration = Duration::from_millis((1000.0 / frequency as f32) as u64);

        tokio::spawn(async move {
            loop {
                let mut bus_lock = bus.lock().await;
                let result = sensor.read(&mut *bus_lock).await;

                match result {
                    Ok(frame) => {
                        println!("[{}] {:?}", sensor_id, frame);
                    }
                    Err(_e) => {
                    }
                }

                sleep(sleep_duration).await;
            }
        });
    }
}
