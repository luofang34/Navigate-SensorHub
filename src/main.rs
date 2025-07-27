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
    println!("[NavigateSensorHub] starting up...");
    let sensor_config = load_sensor_config("config/sensors.toml").expect("Failed to load sensor config");
    println!("[config] loaded {} sensor(s)", sensor_config.sensors.len());
    let (sensors, buses) = init_all(&sensor_config).await.expect("Initialization failed");
    println!("[registry] sensors and buses initialized");
    spawn_sensor_tasks(sensors, buses).await;
    println!("[main] sensor tasks launched — entering idle wait");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
