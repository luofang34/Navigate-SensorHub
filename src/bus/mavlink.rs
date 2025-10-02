use super::serial::SerialBus;
use mavlink;
use tokio::sync::broadcast;
use tracing::{debug, trace, warn, info, error};

/// MAVLink connection wrapper that handles message streaming
pub struct MavlinkConnection {
    /// Broadcast sender for MAVLink messages (can be cloned for multiple subscribers)
    tx: broadcast::Sender<mavlink::common::MavMessage>,
}

impl MavlinkConnection {
    /// Create a new MAVLink connection from a serial bus
    /// Takes ownership of the SerialBus and starts the message loop
    pub fn new(serial: SerialBus) -> Self {
        // Create a broadcast channel with a reasonable buffer (1000 messages)
        let (tx, _rx) = broadcast::channel(1000);

        // Spawn the receive loop
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            // Take ownership of the stream and wrap in AsyncPeekReader
            let stream = serial.into_stream();
            let mut peek_reader = mavlink::async_peek_reader::AsyncPeekReader::new(stream);

            info!("[MAVLink] Starting receive loop...");

            loop {
                match mavlink::read_v2_msg_async::<mavlink::common::MavMessage, _>(&mut peek_reader).await {
                    Ok((header, msg)) => {
                        // Successfully parsed a MAVLink message
                        trace!("[MAVLink] Received message from sys={} comp={}: {:?}",
                               header.system_id, header.component_id, msg);

                        // Debug log for sensor messages
                        match &msg {
                            mavlink::common::MavMessage::SCALED_IMU(imu) => {
                                debug!("[MAVLink] SCALED_IMU: acc=({},{},{}), gyro=({},{},{}), mag=({},{},{})",
                                       imu.xacc, imu.yacc, imu.zacc,
                                       imu.xgyro, imu.ygyro, imu.zgyro,
                                       imu.xmag, imu.ymag, imu.zmag);
                            }
                            mavlink::common::MavMessage::SCALED_PRESSURE(press) => {
                                debug!("[MAVLink] SCALED_PRESSURE: press_abs={}, press_diff={}, temp={}",
                                       press.press_abs, press.press_diff, press.temperature);
                            }
                            mavlink::common::MavMessage::HEARTBEAT(_) => {
                                trace!("[MAVLink] Heartbeat received");
                            }
                            _ => {
                                trace!("[MAVLink] Other message type received");
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

        Self { tx }
    }

    /// Subscribe to MAVLink messages from this connection
    pub fn subscribe(&self) -> broadcast::Receiver<mavlink::common::MavMessage> {
        self.tx.subscribe()
    }
}
