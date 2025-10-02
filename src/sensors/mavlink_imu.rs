use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::bus::mavlink::MavlinkConnection;
use crate::errors::{SensorError, SensorResult};
use async_trait::async_trait;
use mavlink::common::MavMessage;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, trace, info, error};

/// MAVLink IMU sensor driver
/// Subscribes to SCALED_IMU, SCALED_IMU2, SCALED_IMU3, or HIGHRES_IMU messages
pub struct MavlinkImu {
    id: String,
    bus_id: String,
    // Last received sensor data (cached for polling interface)
    last_frame: Arc<Mutex<Option<SensorDataFrame>>>,
    mavlink_conn: Option<Arc<MavlinkConnection>>,
}

impl MavlinkImu {
    pub fn new(id: String, _address: u8, bus_id: String) -> Self {
        Self {
            id,
            bus_id,
            last_frame: Arc::new(Mutex::new(None)),
            mavlink_conn: None,
        }
    }

    /// Set the MAVLink connection for this sensor
    pub fn set_mavlink_connection(&mut self, conn: Arc<MavlinkConnection>) {
        self.mavlink_conn = Some(conn);
    }

    /// Start the message receive loop and update last_frame on new messages
    fn start_message_loop(&self, mut rx: broadcast::Receiver<MavMessage>) {
        let last_frame = self.last_frame.clone();
        let sensor_id = self.id.clone();

        tokio::spawn(async move {
            info!("[{}] Starting MAVLink IMU message loop", sensor_id);
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MavMessage::SCALED_IMU(imu) => {
                        trace!("[{}] Received SCALED_IMU message", sensor_id);
                        let mut frame = SensorDataFrame::default();

                        // Convert from milli-units to SI units
                        // Accelerometer: milli-g to m/s²
                        frame.accel = Some([
                            (imu.xacc as f32 / 1000.0) * 9.81,
                            (imu.yacc as f32 / 1000.0) * 9.81,
                            (imu.zacc as f32 / 1000.0) * 9.81,
                        ]);

                        // Gyroscope: milli-rad/s to rad/s
                        frame.gyro = Some([
                            imu.xgyro as f32 / 1000.0,
                            imu.ygyro as f32 / 1000.0,
                            imu.zgyro as f32 / 1000.0,
                        ]);

                        debug!("[{}] IMU data: accel={:?}, gyro={:?}",
                               sensor_id, frame.accel, frame.gyro);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::HIGHRES_IMU(imu) => {
                        trace!("[{}] Received HIGHRES_IMU message", sensor_id);
                        let mut frame = SensorDataFrame::default();

                        // HIGHRES_IMU provides data in SI units directly
                        frame.accel = Some([imu.xacc, imu.yacc, imu.zacc]);
                        frame.gyro = Some([imu.xgyro, imu.ygyro, imu.zgyro]);
                        frame.temp = Some(imu.temperature);

                        debug!("[{}] HIGHRES_IMU data: accel={:?}, gyro={:?}, temp={}",
                               sensor_id, frame.accel, frame.gyro, imu.temperature);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::SCALED_IMU2(imu) => {
                        let mut frame = SensorDataFrame::default();

                        frame.accel = Some([
                            (imu.xacc as f32 / 1000.0) * 9.81,
                            (imu.yacc as f32 / 1000.0) * 9.81,
                            (imu.zacc as f32 / 1000.0) * 9.81,
                        ]);

                        frame.gyro = Some([
                            imu.xgyro as f32 / 1000.0,
                            imu.ygyro as f32 / 1000.0,
                            imu.zgyro as f32 / 1000.0,
                        ]);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::SCALED_IMU3(imu) => {
                        let mut frame = SensorDataFrame::default();

                        frame.accel = Some([
                            (imu.xacc as f32 / 1000.0) * 9.81,
                            (imu.yacc as f32 / 1000.0) * 9.81,
                            (imu.zacc as f32 / 1000.0) * 9.81,
                        ]);

                        frame.gyro = Some([
                            imu.xgyro as f32 / 1000.0,
                            imu.ygyro as f32 / 1000.0,
                            imu.zgyro as f32 / 1000.0,
                        ]);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }
            error!("[{}] MAVLink message loop ended unexpectedly", sensor_id);
        });
    }
}

#[async_trait]
impl SensorDriver for MavlinkImu {
    async fn init(&mut self, _bus: &mut I2CBus) -> SensorResult<()> {
        // For MAVLink sensors, initialization happens via the connection
        // Subscribe to MAVLink messages if we have a connection
        if let Some(conn) = &self.mavlink_conn {
            let rx = conn.subscribe();
            self.start_message_loop(rx);
            Ok(())
        } else {
            Err(SensorError::InitError {
                sensor: self.id.clone(),
                reason: "No MAVLink connection set".to_string(),
            })
        }
    }

    async fn read(&self, _bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        // Return the last received frame
        let last = self.last_frame.lock().await;
        match last.as_ref() {
            Some(frame) => Ok(frame.clone()),
            None => Err(SensorError::ReadError {
                sensor: self.id.clone(),
                reason: "No data received yet from MAVLink".to_string(),
            }),
        }
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
