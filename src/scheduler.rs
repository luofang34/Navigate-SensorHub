use crate::sensors::SensorDriver;
use crate::bus::i2c::I2CBus;
use crate::config::sensor_config::SensorConfig;
use crate::messages::{Header, ImuMessage, MagnetometerMessage, BarometerMessage, SensorMessage};
use crate::grpc_service::SensorHubService;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub async fn spawn_sensor_tasks(
    sensors: Vec<Box<dyn SensorDriver>>,
    buses: HashMap<String, Arc<Mutex<I2CBus>>>,
    grpc_service: Arc<SensorHubService>,
    sensor_config: &SensorConfig,
) {
    
    for sensor in sensors.into_iter() {
        let sensor_id = sensor.id().to_string();
        let bus = buses.get(sensor.bus()).unwrap().clone();
        
        // Find the sensor configuration to get frequency
        let frequency = sensor_config.sensors
            .iter()
            .find(|s| s.id == sensor_id)
            .and_then(|s| s.frequency)
            .unwrap_or(100); // Default to 100Hz if not specified
        let sleep_duration = Duration::from_millis((1000.0 / frequency as f32) as u64);
        let grpc_service_clone = grpc_service.clone();
        let mut sequence_counter = 0u64;

        tokio::spawn(async move {
            println!("[{}] Starting sensor task at {}Hz", sensor_id, frequency);
            
            loop {
                let mut bus_lock = bus.lock().await;
                let result = sensor.read(&mut *bus_lock).await;
                drop(bus_lock); // Release lock early

                match result {
                    Ok(frame) => {
                        sequence_counter += 1;
                        
                        // Create header with timing metadata
                        let header = Header::new(
                            "navigate_hub".to_string(),
                            sensor_id.clone(),
                            "sensor_frame".to_string(),
                            sequence_counter,
                        );
                        
                        // Convert SensorDataFrame to appropriate message type based on data present
                        let mut messages = Vec::new();
                        
                        // IMU data (accelerometer + gyroscope)
                        if let (Some(accel), Some(gyro)) = (frame.accel, frame.gyro) {
                            let imu_msg = ImuMessage {
                                h: header.clone(),
                                ax: accel[0], ay: accel[1], az: accel[2],
                                gx: gyro[0], gy: gyro[1], gz: gyro[2],
                            };
                            messages.push(SensorMessage::Imu(imu_msg));
                        }
                        
                        // Magnetometer data
                        if let Some(mag) = frame.mag {
                            let mag_msg = MagnetometerMessage {
                                h: header.clone(),
                                mx: mag[0], my: mag[1], mz: mag[2],
                            };
                            messages.push(SensorMessage::Magnetometer(mag_msg));
                        }
                        
                        // Barometer data (use static pressure primarily)
                        if let Some(pressure) = frame.pressure_static.or(frame.pressure_pitot) {
                            let temperature = frame.temp.unwrap_or(20.0); // Default 20°C
                            
                            // Calculate altitude using standard atmosphere (ISA)
                            // h = 44330 * (1 - (P/P0)^0.1903)
                            let altitude = if pressure > 0.0 {
                                44330.0 * (1.0 - (pressure / 101325.0).powf(0.1903))
                            } else {
                                0.0
                            };
                            
                            let baro_msg = BarometerMessage {
                                h: header.clone(),
                                pressure,
                                temperature,
                                altitude,
                            };
                            messages.push(SensorMessage::Barometer(baro_msg));
                        }
                        
                        // Publish all messages to gRPC service
                        for msg in messages {
                            if let Err(e) = grpc_service_clone.publish(msg).await {
                                eprintln!("[{}] Failed to publish: {}", sensor_id, e);
                            }
                        }
                        
                    }
                    Err(e) => {
                        eprintln!("[{}] Sensor read error: {}", sensor_id, e);
                    }
                }

                sleep(sleep_duration).await;
            }
        });
    }
}
