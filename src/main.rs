mod registry;
mod scheduler;
mod config;
mod bus;
mod sensors;

use crate::config::load_sensor_config;
use crate::registry::init_all;
use crate::scheduler::spawn_sensor_tasks;

#[tokio::main]
async fn main() {
    let sensor_config = load_sensor_config("config/sensors.toml").expect("Failed to load sensor config");
    let (sensors, buses) = init_all(&sensor_config).await.expect("Initialization failed");
    spawn_sensor_tasks(sensors, buses, &sensor_config).await;
}
