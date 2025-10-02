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

/// MAVLink Barometer sensor driver
/// Subscribes to SCALED_PRESSURE or RAW_PRESSURE messages
pub struct MavlinkBaro {
    id: String,
    bus_id: String,
    // Last received sensor data (cached for polling interface)
    last_frame: Arc<Mutex<Option<SensorDataFrame>>>,
    mavlink_conn: Option<Arc<MavlinkConnection>>,
}

impl MavlinkBaro {
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
            info!("[{}] Starting MAVLink Barometer message loop", sensor_id);
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MavMessage::SCALED_PRESSURE(pressure) => {
                        trace!("[{}] Received SCALED_PRESSURE message", sensor_id);
                        let mut frame = SensorDataFrame::default();

                        // SCALED_PRESSURE provides pressure in hectopascals (hPa)
                        // Convert to Pascals
                        frame.pressure_static = Some(pressure.press_abs * 100.0);

                        // Differential pressure (if available)
                        if pressure.press_diff != 0.0 {
                            frame.pressure_pitot = Some(pressure.press_diff * 100.0);
                        }

                        // Temperature in centi-degrees Celsius
                        frame.temp = Some(pressure.temperature as f32 / 100.0);

                        debug!("[{}] Baro data: press_abs={} Pa, temp={} °C",
                               sensor_id, frame.pressure_static.unwrap(), frame.temp.unwrap());

                        let mut last = last_frame.lock().await;
                        *last = Some(frame);
                    }
                    MavMessage::RAW_PRESSURE(pressure) => {
                        let mut frame = SensorDataFrame::default();

                        // RAW_PRESSURE is typically not used for barometric altitude
                        // but we can still extract the values if needed
                        // Values are typically in Pascals already for press_abs
                        frame.pressure_static = Some(pressure.press_abs as f32);

                        if pressure.press_diff1 != 0 {
                            frame.pressure_pitot = Some(pressure.press_diff1 as f32);
                        }

                        // Temperature in centi-degrees Celsius
                        frame.temp = Some(pressure.temperature as f32 / 100.0);

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
impl SensorDriver for MavlinkBaro {
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
