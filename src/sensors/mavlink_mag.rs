use super::{SensorDataFrame, SensorDriver};
use crate::bus::i2c::I2CBus;
use crate::bus::mavlink::MavlinkConnection;
use crate::errors::{SensorError, SensorResult};
use async_trait::async_trait;
use mavlink::common::MavMessage;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

/// MAVLink Magnetometer sensor driver
/// Subscribes to SCALED_IMU (which includes magnetometer) or HIL_SENSOR messages
pub struct MavlinkMag {
    id: String,
    bus_id: String,
    // Last received sensor data (cached for polling interface)
    last_frame: Arc<Mutex<Option<SensorDataFrame>>>,
    mavlink_conn: Option<Arc<MavlinkConnection>>,
}

impl MavlinkMag {
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
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MavMessage::SCALED_IMU(imu) => {
                        let mut frame = SensorDataFrame::default();

                        // Magnetometer data in milli-Gauss, convert to micro-Tesla
                        // 1 Gauss = 100 micro-Tesla
                        // 1 milli-Gauss = 0.1 micro-Tesla
                        frame.mag = Some([
                            imu.xmag as f32 * 0.1,
                            imu.ymag as f32 * 0.1,
                            imu.zmag as f32 * 0.1,
                        ]);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::HIGHRES_IMU(imu) => {
                        let mut frame = SensorDataFrame::default();

                        // HIGHRES_IMU provides magnetometer data in Gauss
                        // Convert to micro-Tesla (1 Gauss = 100 μT)
                        frame.mag = Some([
                            imu.xmag * 100.0,
                            imu.ymag * 100.0,
                            imu.zmag * 100.0,
                        ]);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::SCALED_IMU2(imu) => {
                        let mut frame = SensorDataFrame::default();

                        frame.mag = Some([
                            imu.xmag as f32 * 0.1,
                            imu.ymag as f32 * 0.1,
                            imu.zmag as f32 * 0.1,
                        ]);

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::SCALED_IMU3(imu) => {
                        let mut frame = SensorDataFrame::default();

                        frame.mag = Some([
                            imu.xmag as f32 * 0.1,
                            imu.ymag as f32 * 0.1,
                            imu.zmag as f32 * 0.1,
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
impl SensorDriver for MavlinkMag {
    async fn init(&mut self, _bus: &mut I2CBus) -> SensorResult<()> {
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
