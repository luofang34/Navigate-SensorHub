use crate::bus::i2c::I2CBus;
use crate::bus::mavlink::{DetectedSensor, MavlinkConnection};
use crate::bus::serial::SerialBus;
use crate::bus::BusType;
use crate::config::load_bus_config;
use crate::config::sensor_config::SensorConfig;
use crate::errors::{ConfigError, RegistryError, RegistryResult, SensorError};
use crate::grpc_service::SensorHubService;
use crate::sensors::create_sensor_driver;
use crate::sensors::SensorDriver;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Create a MAVLink sensor driver from detected sensor type with proper instance mapping
async fn create_mavlink_sensor(
    sensor_type: DetectedSensor,
    bus_id: &str,
    mavlink_conn: &Arc<MavlinkConnection>,
    grpc_service: &Arc<SensorHubService>,
    dummy_i2c_bus: Option<&Arc<Mutex<I2CBus>>>,
) -> RegistryResult<Box<dyn SensorDriver>> {
    #[cfg(feature = "mavlink_sensors")]
    {
        use crate::sensors::mavlink::MavlinkSensorType;

        // Map DetectedSensor enum to (id, MavlinkSensorType)
        let (id, mavlink_type) = match sensor_type {
            DetectedSensor::ScaledImu => (
                "fc_imu0".to_string(),
                MavlinkSensorType::Imu { instance: 0 },
            ),
            DetectedSensor::ScaledImu2 => (
                "fc_imu1".to_string(),
                MavlinkSensorType::Imu { instance: 1 },
            ),
            DetectedSensor::ScaledImu3 => (
                "fc_imu2".to_string(),
                MavlinkSensorType::Imu { instance: 2 },
            ),
            DetectedSensor::HighresImu => {
                ("fc_imu_highres".to_string(), MavlinkSensorType::HighresImu)
            }
            DetectedSensor::ScaledPressure => {
                ("fc_baro0".to_string(), MavlinkSensorType::Barometer)
            }
            DetectedSensor::AttitudeQuaternion => {
                ("fc_attitude".to_string(), MavlinkSensorType::Attitude)
            }
        };

        info!(
            "[registry] Auto-creating MAVLink sensor: {} (type: {:?})",
            id, mavlink_type
        );

        // Create MavlinkSensor directly with the correct type (bypass factory)
        use crate::sensors::mavlink::MavlinkSensor;
        let mut sensor: Box<dyn SensorDriver> = Box::new(MavlinkSensor::new(
            id.clone(),
            bus_id.to_string(),
            mavlink_type,
        ));

        // Inject gRPC service and MAVLink connection
        if let Some(mavlink_sensor) = sensor.as_any_mut().downcast_mut::<MavlinkSensor>() {
            mavlink_sensor.set_grpc_service(grpc_service.clone());
            mavlink_sensor.set_mavlink_connection(mavlink_conn.clone());
            info!(
                "[registry] Injected gRPC service and MAVLink connection into {}",
                id
            );
        } else {
            warn!(
                "[registry] Failed to downcast sensor {} to MavlinkSensor - this shouldn't happen!",
                id
            );
        }

        // Initialize the sensor (for MAVLink sensors, this is a no-op - message loop already started)
        if let Some(i2c_bus) = dummy_i2c_bus {
            let mut bus = i2c_bus.lock().await;
            sensor
                .init(&mut bus)
                .await
                .map_err(RegistryError::RegistrationError)?;
        }

        Ok(sensor)
    }

    #[cfg(not(feature = "mavlink_sensors"))]
    {
        Err(RegistryError::DriverCreationError(
            SensorError::UnsupportedDriver {
                driver: "mavlink_sensors feature not enabled".to_string(),
            },
        ))
    }
}

pub async fn init_all(
    sensor_config: &SensorConfig,
    grpc_service: Arc<SensorHubService>,
) -> RegistryResult<(
    Vec<Box<dyn SensorDriver>>,
    HashMap<String, Arc<Mutex<I2CBus>>>,
)> {
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let bus_config_path = format!("{}/buses.toml", config_path);
    let bus_cfg = load_bus_config(&bus_config_path).map_err(|e| {
        RegistryError::BusInitError(ConfigError::LoadError {
            path: bus_config_path.clone(),
            source: e,
        })
    })?;

    let mut i2c_bus_map = HashMap::new();
    let mut mavlink_connections: HashMap<String, Arc<MavlinkConnection>> = HashMap::new();

    // Initialize buses based on type
    for b in bus_cfg.buses.iter() {
        let bus_type = BusType::from_str(&b.r#type).ok_or_else(|| {
            RegistryError::BusInitError(ConfigError::ValidationError(format!(
                "Unknown bus type: {}",
                b.r#type
            )))
        })?;

        match bus_type {
            BusType::I2C => {
                info!("[registry] Initializing I2C bus: {} at {}", b.id, b.path);
                match I2CBus::new(&b.path) {
                    Ok(bus) => {
                        i2c_bus_map.insert(b.id.clone(), Arc::new(Mutex::new(bus)));
                        info!("[registry] I2C bus {} initialized successfully", b.id);
                    }
                    Err(e) => {
                        warn!("[registry] Failed to initialize I2C bus {} (this is OK on macOS if only using MAVLink): {:?}", b.id, e);
                        // Continue - MAVLink sensors don't actually need I2C
                    }
                }
            }
            BusType::Serial => {
                // Check if auto-detection is requested
                let (serial, auto_detect) = if b.path.trim() == "auto" {
                    info!(
                        "[registry] Auto-detecting flight controller for Serial/MAVLink bus: {}",
                        b.id
                    );

                    // Retry auto-detection with backoff if no FC found initially
                    let mut backoff_ms = 100u64;
                    const MAX_BACKOFF_MS: u64 = 2000;
                    let detected_path = loop {
                        match SerialBus::detect_flight_controller().await {
                            Ok(path) => {
                                info!(
                                    "[registry] Flight controller auto-detected at: {}",
                                    path
                                );
                                break path;
                            }
                            Err(e) => {
                                warn!(
                                    "[registry] Flight controller not found ({}), retrying in {}ms...",
                                    e, backoff_ms
                                );
                                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms))
                                    .await;
                                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                            }
                        }
                    };

                    let serial = SerialBus::new(&detected_path).map_err(|e| {
                        error!("[registry] Failed to open serial port {}: {}", detected_path, e);
                        RegistryError::DriverCreationError(SensorError::SerialError(e.into()))
                    })?;
                    (serial, true)
                } else {
                    info!(
                        "[registry] Initializing Serial/MAVLink bus: {} at {}",
                        b.id, b.path
                    );
                    let serial = SerialBus::new(&b.path).map_err(|e| {
                        error!("[registry] Failed to open serial port {}: {}", b.path, e);
                        RegistryError::DriverCreationError(SensorError::SerialError(e.into()))
                    })?;
                    (serial, false)
                };

                // Log which port was successfully opened (useful for multi-machine testing)
                let port_path = serial.path().to_string();
                let mavlink_conn = MavlinkConnection::new(serial, auto_detect);
                mavlink_connections.insert(b.id.clone(), Arc::new(mavlink_conn));
                info!(
                    "[registry] Serial/MAVLink bus {} initialized successfully on {}",
                    b.id, port_path
                );
            }
        }
    }

    let mut sensors: Vec<Box<dyn SensorDriver>> = Vec::new();

    // First, initialize locally-connected sensors (I2C, SPI, etc.) from config
    let local_sensors: Vec<_> = sensor_config
        .sensors
        .iter()
        .filter(|s| !s.driver.starts_with("mavlink_"))
        .collect();

    info!(
        "[registry] Initializing {} local sensor(s) from config...",
        local_sensors.len()
    );

    for s in local_sensors.iter() {
        debug!(
            "[registry] Creating sensor driver: id={} type={} bus={} addr=0x{:02X}",
            s.id, s.driver, s.bus, s.address
        );
        let mut sensor = create_sensor_driver(&s.driver, s.id.clone(), s.address, s.bus.clone())
            .map_err(|e| {
                error!("[registry] Failed to create sensor {}: {:?}", s.id, e);
                RegistryError::DriverCreationError(e)
            })?;

        // For I2C sensors, use the I2C bus
        let bus_arc = i2c_bus_map.get(&s.bus).ok_or_else(|| {
            RegistryError::DriverCreationError(SensorError::BusNotFound { bus: s.bus.clone() })
        })?;
        let mut bus = bus_arc.lock().await;
        sensor
            .init(&mut bus)
            .await
            .map_err(RegistryError::RegistrationError)?;

        info!("[registry] Local sensor {} created successfully", s.id);
        sensors.push(sensor);
    }

    // Auto-discover MAVLink sensors from each serial bus
    for (bus_id, mavlink_conn) in mavlink_connections.iter() {
        info!(
            "[registry] Waiting for MAVLink sensor auto-discovery on bus {}...",
            bus_id
        );

        // Wait a short time for sensors to be detected (messages to arrive)
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let detected = mavlink_conn.get_detected_sensors().await;
        info!(
            "[registry] Auto-detected {} MAVLink sensor type(s) on bus {}",
            detected.len(),
            bus_id
        );

        // Get a dummy I2C bus for initialization (MAVLink sensors don't actually use it)
        let dummy_bus = i2c_bus_map.values().next();

        for sensor_type in detected {
            match create_mavlink_sensor(sensor_type, bus_id, mavlink_conn, &grpc_service, dummy_bus)
                .await
            {
                Ok(sensor) => {
                    info!(
                        "[registry] MAVLink sensor {} created successfully",
                        sensor.id()
                    );
                    sensors.push(sensor);
                }
                Err(e) => {
                    error!(
                        "[registry] Failed to create MAVLink sensor {:?}: {:?}",
                        sensor_type, e
                    );
                }
            }
        }
    }

    info!("[registry] Total sensors initialized: {}", sensors.len());
    Ok((sensors, i2c_bus_map))
}
