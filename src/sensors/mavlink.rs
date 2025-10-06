use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::bus::mavlink::MavlinkConnection;
use crate::errors::{SensorError, SensorResult};
use crate::grpc_service::SensorHubService;
use crate::messages::{BarometerMessage, Header, ImuMessage, SensorMessage};
use async_trait::async_trait;
use mavlink::common::MavMessage;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace};

/// MAVLink sensor type enum - defines which message type this sensor processes
///
/// See bus/mavlink.rs TODO for implementation guidance on adding new message types.
#[derive(Clone, Debug, PartialEq)]
pub enum MavlinkSensorType {
    /// IMU sensor with instance number (0=SCALED_IMU, 1=SCALED_IMU2, 2=SCALED_IMU3)
    Imu { instance: u8 },
    /// High-resolution IMU (HIGHRES_IMU message)
    HighresImu,
    /// Barometer (SCALED_PRESSURE message)
    Barometer,
    /// Attitude quaternion (ATTITUDE_QUATERNION message)
    Attitude,
}

/// Unified MAVLink sensor - handles all MAVLink message types
/// Each sensor subscribes to the MAVLink broadcast and filters for its specific message type
pub struct MavlinkSensor {
    id: String,
    bus_id: String,
    sensor_type: MavlinkSensorType,
    grpc_service: Option<Arc<SensorHubService>>,
    mavlink_conn: Option<Arc<MavlinkConnection>>,
    sequence_counter: Arc<Mutex<u64>>,
}

impl MavlinkSensor {
    pub fn new(id: String, bus_id: String, sensor_type: MavlinkSensorType) -> Self {
        Self {
            id,
            bus_id,
            sensor_type,
            grpc_service: None,
            mavlink_conn: None,
            sequence_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Set the gRPC service for publishing sensor data
    pub fn set_grpc_service(&mut self, service: Arc<SensorHubService>) {
        self.grpc_service = Some(service);
    }

    /// Set the MAVLink connection and start the message loop
    pub fn set_mavlink_connection(&mut self, conn: Arc<MavlinkConnection>) {
        self.mavlink_conn = Some(conn.clone());
        let rx = conn.subscribe();
        self.start_message_loop(rx);
    }

    /// Start the message receive loop - publishes directly to gRPC
    fn start_message_loop(&self, mut rx: broadcast::Receiver<MavMessage>) {
        let grpc = self
            .grpc_service
            .clone()
            .expect("gRPC service must be set before starting message loop");
        let sensor_type = self.sensor_type.clone();
        let sensor_id = self.id.clone();
        let seq = self.sequence_counter.clone();

        tokio::spawn(async move {
            info!(
                "[{}] Starting MAVLink message loop for {:?}",
                sensor_id, sensor_type
            );

            while let Ok(msg) = rx.recv().await {
                // Match on BOTH sensor type AND message type - only process matching pairs
                let frame_opt = match (&sensor_type, &msg) {
                    // IMU instance 0 - SCALED_IMU
                    (MavlinkSensorType::Imu { instance: 0 }, MavMessage::SCALED_IMU(imu)) => {
                        trace!("[{}] Received SCALED_IMU", sensor_id);
                        Some(convert_scaled_imu_to_frame(imu))
                    }
                    // IMU instance 1 - SCALED_IMU2
                    (MavlinkSensorType::Imu { instance: 1 }, MavMessage::SCALED_IMU2(imu)) => {
                        trace!("[{}] Received SCALED_IMU2", sensor_id);
                        Some(convert_scaled_imu2_to_frame(imu))
                    }
                    // IMU instance 2 - SCALED_IMU3
                    (MavlinkSensorType::Imu { instance: 2 }, MavMessage::SCALED_IMU3(imu)) => {
                        trace!("[{}] Received SCALED_IMU3", sensor_id);
                        Some(convert_scaled_imu3_to_frame(imu))
                    }
                    // High-resolution IMU
                    (MavlinkSensorType::HighresImu, MavMessage::HIGHRES_IMU(imu)) => {
                        trace!("[{}] Received HIGHRES_IMU", sensor_id);
                        Some(convert_highres_imu_to_frame(imu))
                    }
                    // Barometer
                    (MavlinkSensorType::Barometer, MavMessage::SCALED_PRESSURE(p)) => {
                        trace!("[{}] Received SCALED_PRESSURE", sensor_id);
                        Some(convert_pressure_to_frame(p))
                    }
                    // Attitude
                    (MavlinkSensorType::Attitude, MavMessage::ATTITUDE_QUATERNION(att)) => {
                        trace!("[{}] Received ATTITUDE_QUATERNION", sensor_id);
                        Some(convert_attitude_to_frame(att))
                    }
                    _ => None, // Not for this sensor instance
                };

                if let Some(frame) = frame_opt {
                    // Increment sequence counter
                    let mut seq_lock = seq.lock().await;
                    *seq_lock += 1;
                    let seq_num = *seq_lock;
                    drop(seq_lock);

                    // Create header with timing metadata
                    let header = Header::new(
                        "navigate_hub".to_string(),
                        sensor_id.clone(),
                        "sensor_frame".to_string(),
                        seq_num,
                    );

                    // Convert frame to gRPC messages and publish
                    let messages = frame_to_grpc_messages(frame, header, &sensor_id);
                    for msg in messages {
                        if let Err(e) = grpc.publish(msg).await {
                            error!("[{}] Failed to publish: {}", sensor_id, e);
                        }
                    }
                }
            }

            error!("[{}] MAVLink message loop ended unexpectedly", sensor_id);
        });
    }
}

/// Convert SCALED_IMU data to SensorDataFrame
fn convert_scaled_imu_to_frame(imu: &mavlink::common::SCALED_IMU_DATA) -> SensorDataFrame {
    SensorDataFrame {
        accel: Some([
            (imu.xacc as f32 / 1000.0) * 9.81, // milli-g to m/s²
            (imu.yacc as f32 / 1000.0) * 9.81,
            (imu.zacc as f32 / 1000.0) * 9.81,
        ]),
        gyro: Some([
            imu.xgyro as f32 / 1000.0, // milli-rad/s to rad/s
            imu.ygyro as f32 / 1000.0,
            imu.zgyro as f32 / 1000.0,
        ]),
        // Note: SCALED_IMU has no temperature field
        ..Default::default()
    }
}

/// Convert SCALED_IMU2 data to SensorDataFrame
fn convert_scaled_imu2_to_frame(imu: &mavlink::common::SCALED_IMU2_DATA) -> SensorDataFrame {
    SensorDataFrame {
        accel: Some([
            (imu.xacc as f32 / 1000.0) * 9.81, // milli-g to m/s²
            (imu.yacc as f32 / 1000.0) * 9.81,
            (imu.zacc as f32 / 1000.0) * 9.81,
        ]),
        gyro: Some([
            imu.xgyro as f32 / 1000.0, // milli-rad/s to rad/s
            imu.ygyro as f32 / 1000.0,
            imu.zgyro as f32 / 1000.0,
        ]),
        ..Default::default()
    }
}

/// Convert SCALED_IMU3 data to SensorDataFrame
fn convert_scaled_imu3_to_frame(imu: &mavlink::common::SCALED_IMU3_DATA) -> SensorDataFrame {
    SensorDataFrame {
        accel: Some([
            (imu.xacc as f32 / 1000.0) * 9.81, // milli-g to m/s²
            (imu.yacc as f32 / 1000.0) * 9.81,
            (imu.zacc as f32 / 1000.0) * 9.81,
        ]),
        gyro: Some([
            imu.xgyro as f32 / 1000.0, // milli-rad/s to rad/s
            imu.ygyro as f32 / 1000.0,
            imu.zgyro as f32 / 1000.0,
        ]),
        ..Default::default()
    }
}

/// Convert HIGHRES_IMU data to SensorDataFrame
fn convert_highres_imu_to_frame(imu: &mavlink::common::HIGHRES_IMU_DATA) -> SensorDataFrame {
    SensorDataFrame {
        accel: Some([imu.xacc, imu.yacc, imu.zacc]), // Already in m/s²
        gyro: Some([imu.xgyro, imu.ygyro, imu.zgyro]), // Already in rad/s
        temp: Some(imu.temperature),                 // Already in °C
        ..Default::default()
    }
}

/// Convert SCALED_PRESSURE data to SensorDataFrame
fn convert_pressure_to_frame(p: &mavlink::common::SCALED_PRESSURE_DATA) -> SensorDataFrame {
    SensorDataFrame {
        pressure_static: Some(p.press_abs * 100.0), // hPa to Pa
        pressure_pitot: if p.press_diff != 0.0 {
            Some(p.press_diff * 100.0) // hPa to Pa
        } else {
            None
        },
        temp: Some(p.temperature as f32 / 100.0), // centi-degrees to °C
        ..Default::default()
    }
}

/// Convert ATTITUDE_QUATERNION data to SensorDataFrame
fn convert_attitude_to_frame(att: &mavlink::common::ATTITUDE_QUATERNION_DATA) -> SensorDataFrame {
    SensorDataFrame {
        quaternion: Some([att.q1, att.q2, att.q3, att.q4]), // w, x, y, z
        angular_velocity_body: Some([att.rollspeed, att.pitchspeed, att.yawspeed]), // rad/s
        ..Default::default()
    }
}

/// Convert SensorDataFrame to gRPC messages
fn frame_to_grpc_messages(
    frame: SensorDataFrame,
    header: Header,
    sensor_id: &str,
) -> Vec<SensorMessage> {
    let mut messages = Vec::new();

    // IMU data (accelerometer + gyroscope)
    if let (Some(accel), Some(gyro)) = (frame.accel, frame.gyro) {
        let imu_msg = ImuMessage {
            h: header.clone(),
            ax: accel[0],
            ay: accel[1],
            az: accel[2],
            gx: gyro[0],
            gy: gyro[1],
            gz: gyro[2],
        };
        messages.push(SensorMessage::Imu(imu_msg));
        debug!(
            "[{}] Publishing IMU: accel={:?}, gyro={:?}",
            sensor_id, accel, gyro
        );
    }

    // Barometer data
    if let Some(pressure) = frame.pressure_static.or(frame.pressure_pitot) {
        let temperature = frame.temp.unwrap_or(20.0);

        // Calculate altitude using standard atmosphere (ISA): h = 44330 * (1 - (P/P0)^0.1903)
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
        debug!(
            "[{}] Publishing Baro: press={:.1} Pa, temp={:.1}°C, alt={:.1}m",
            sensor_id, pressure, temperature, altitude
        );
    }

    // Note: Attitude quaternion data is currently dropped - add Attitude message type
    // to messages.rs if needed (see bus/mavlink.rs TODO for adding new message types)

    messages
}

/// Implement SensorDriver trait for compatibility
/// Note: MAVLink sensors don't support polling - they're push-based
#[async_trait]
impl SensorDriver for MavlinkSensor {
    async fn init(&mut self, _bus: &mut I2CBus) -> SensorResult<()> {
        // MAVLink sensors initialize via set_mavlink_connection()
        // Message loop starts there, so this is a no-op
        if self.grpc_service.is_some() && self.mavlink_conn.is_some() {
            Ok(())
        } else {
            Err(SensorError::InitError {
                sensor: self.id.clone(),
                reason: "gRPC service or MAVLink connection not set".to_string(),
            })
        }
    }

    async fn read(&self, _bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        // MAVLink sensors don't support polling - they're push-based
        // Data is published directly to gRPC from the message loop
        Err(SensorError::ReadError {
            sensor: self.id.clone(),
            reason: "MAVLink sensors are push-based, data published via gRPC stream".to_string(),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn bus(&self) -> &str {
        &self.bus_id
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
