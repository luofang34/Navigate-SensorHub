use super::serial::SerialBus;
use mavlink;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashSet;
use tracing::{debug, trace, warn, info, error};

/// Detected sensor types from MAVLink stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectedSensor {
    ScaledImu,
    ScaledImu2,
    ScaledImu3,
    HighresImu,
    ScaledPressure,
    AttitudeQuaternion,
}

/// MAVLink connection wrapper that handles message streaming
pub struct MavlinkConnection {
    /// Broadcast sender for MAVLink messages (can be cloned for multiple subscribers)
    tx: broadcast::Sender<mavlink::common::MavMessage>,
    /// Set of detected sensors
    detected_sensors: Arc<Mutex<HashSet<DetectedSensor>>>,
}

impl MavlinkConnection {
    /// Create a new MAVLink connection from a serial bus
    /// Takes ownership of the SerialBus and starts the message loop
    pub fn new(serial: SerialBus) -> Self {
        // Create a broadcast channel with a reasonable buffer (1000 messages)
        let (tx, _rx) = broadcast::channel(1000);
        let detected_sensors = Arc::new(Mutex::new(HashSet::new()));

        // Spawn the receive loop
        let tx_clone = tx.clone();
        let detected_clone = detected_sensors.clone();
        tokio::spawn(async move {
            // Take ownership of the stream and wrap in AsyncPeekReader
            let stream = serial.into_stream();
            let mut peek_reader = mavlink::async_peek_reader::AsyncPeekReader::new(stream);

            info!("[MAVLink] Starting receive loop...");

            loop {
                // Auto-detect MAVLink v1 (0xFE) or v2 (0xFD) protocol version
                match mavlink::read_versioned_msg_async::<mavlink::common::MavMessage, _>(
                    &mut peek_reader,
                    mavlink::ReadVersion::Any
                ).await {
                    Ok((header, msg)) => {
                        // Successfully parsed a MAVLink message (auto-detected version)
                        trace!("[MAVLink] Received message from sys={} comp={}: {:?}",
                               header.system_id, header.component_id, msg);

                        // Auto-detect sensors and log
                        let sensor_type = match &msg {
                            mavlink::common::MavMessage::SCALED_IMU(imu) => {
                                debug!("[MAVLink] SCALED_IMU: acc=({},{},{}), gyro=({},{},{}), mag=({},{},{})",
                                       imu.xacc, imu.yacc, imu.zacc,
                                       imu.xgyro, imu.ygyro, imu.zgyro,
                                       imu.xmag, imu.ymag, imu.zmag);
                                Some(DetectedSensor::ScaledImu)
                            }
                            mavlink::common::MavMessage::SCALED_IMU2(imu) => {
                                debug!("[MAVLink] SCALED_IMU2: acc=({},{},{}), gyro=({},{},{}), mag=({},{},{})",
                                       imu.xacc, imu.yacc, imu.zacc,
                                       imu.xgyro, imu.ygyro, imu.zgyro,
                                       imu.xmag, imu.ymag, imu.zmag);
                                Some(DetectedSensor::ScaledImu2)
                            }
                            mavlink::common::MavMessage::SCALED_IMU3(imu) => {
                                debug!("[MAVLink] SCALED_IMU3: acc=({},{},{}), gyro=({},{},{}), mag=({},{},{})",
                                       imu.xacc, imu.yacc, imu.zacc,
                                       imu.xgyro, imu.ygyro, imu.zgyro,
                                       imu.xmag, imu.ymag, imu.zmag);
                                Some(DetectedSensor::ScaledImu3)
                            }
                            mavlink::common::MavMessage::SCALED_PRESSURE(press) => {
                                debug!("[MAVLink] SCALED_PRESSURE: press_abs={}, press_diff={}, temp={}",
                                       press.press_abs, press.press_diff, press.temperature);
                                Some(DetectedSensor::ScaledPressure)
                            }
                            mavlink::common::MavMessage::ATTITUDE_QUATERNION(att) => {
                                debug!("[MAVLink] ATTITUDE_QUATERNION: q=({},{},{},{}), rates=({},{},{})",
                                       att.q1, att.q2, att.q3, att.q4,
                                       att.rollspeed, att.pitchspeed, att.yawspeed);
                                Some(DetectedSensor::AttitudeQuaternion)
                            }
                            mavlink::common::MavMessage::HIGHRES_IMU(imu) => {
                                debug!("[MAVLink] HIGHRES_IMU: acc=({},{},{}), gyro=({},{},{})",
                                       imu.xacc, imu.yacc, imu.zacc,
                                       imu.xgyro, imu.ygyro, imu.zgyro);
                                Some(DetectedSensor::HighresImu)
                            }
                            mavlink::common::MavMessage::HEARTBEAT(_) => {
                                trace!("[MAVLink] Heartbeat received");
                                None
                            }
                            _ => {
                                trace!("[MAVLink] Other message type received");
                                None
                            }
                        };

                        // Track newly detected sensors
                        if let Some(sensor) = sensor_type {
                            let mut detected = detected_clone.lock().await;
                            if detected.insert(sensor) {
                                info!("[MAVLink] Auto-detected new sensor: {:?}", sensor);
                            }
                        }

                        // Broadcast to subscribers
                        match tx_clone.send(msg) {
                            Ok(n) => trace!("[MAVLink] Broadcast to {} receivers", n),
                            Err(_) => trace!("[MAVLink] No active receivers"),
                        }
                    }
                    Err(e) => {
                        match e {
                            mavlink::error::MessageReadError::Io(io_err) => {
                                error!("[MAVLink] I/O error: {}", io_err);
                                // Connection lost, wait a bit and continue
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                continue;
                            }
                            mavlink::error::MessageReadError::Parse(parse_err) => {
                                warn!("[MAVLink] Parse error (skipping): {:?}", parse_err);
                            }
                        }
                    }
                }

                // Small yield to prevent tight loop
                tokio::task::yield_now().await;
            }
        });

        Self { tx, detected_sensors }
    }

    /// Subscribe to MAVLink messages from this connection
    pub fn subscribe(&self) -> broadcast::Receiver<mavlink::common::MavMessage> {
        self.tx.subscribe()
    }

    /// Get the list of detected sensors
    pub async fn get_detected_sensors(&self) -> Vec<DetectedSensor> {
        let detected = self.detected_sensors.lock().await;
        detected.iter().copied().collect()
    }
}
