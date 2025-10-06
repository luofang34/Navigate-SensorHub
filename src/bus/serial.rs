use mavlink::common::MavAutopilot;
use std::io;
use std::time::Duration;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, info, warn};

/// Serial port wrapper for async communication
pub struct SerialBus {
    port: SerialStream,
    /// Port path - useful for logging, error messages, and reconnection logic
    path: String,
}

impl SerialBus {
    /// Create a new serial bus connection
    /// Default baud rate: 57600 (common for MAVLink)
    pub fn new(path: &str) -> io::Result<Self> {
        Self::new_with_baud(path, 57600)
    }

    /// Create a new serial bus connection with custom baud rate
    pub fn new_with_baud(path: &str, baud_rate: u32) -> io::Result<Self> {
        let port = tokio_serial::new(path, baud_rate).open_native_async()?;

        Ok(Self {
            port,
            path: path.to_string(),
        })
    }

    /// Get the port path (useful for logging, debugging, and multi-machine testing)
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Consume self and return the underlying SerialStream
    pub fn into_stream(self) -> SerialStream {
        self.port
    }

    /// Auto-detect flight controller(s) by probing serial ports in parallel for MAVLink HEARTBEAT messages
    /// Returns the path of the first device that responds with a valid flight controller heartbeat
    ///
    /// Note: Probes all ports simultaneously for fastest detection (important for reconnection speed)
    pub async fn detect_flight_controller() -> io::Result<String> {
        let all_fcs = Self::detect_all_flight_controllers().await?;
        all_fcs.into_iter().next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No flight controller found on any serial port",
            )
        })
    }

    /// Auto-detect all flight controllers by probing serial ports in parallel
    /// Returns a vector of all detected FC paths (for future multi-FC redundancy support)
    pub async fn detect_all_flight_controllers() -> io::Result<Vec<String>> {
        info!("[SerialBus] Starting flight controller auto-detection...");

        let ports = tokio_serial::available_ports().map_err(|e| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Failed to enumerate serial ports: {}", e),
            )
        })?;

        if ports.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No serial ports found on system",
            ));
        }

        info!(
            "[SerialBus] Found {} serial port(s), probing in parallel for flight controllers...",
            ports.len()
        );

        // Filter out known non-FC devices
        let candidate_ports: Vec<_> = ports
            .into_iter()
            .filter(|port_info| {
                let port_name = &port_info.port_name;
                let skip = port_name.contains("Bluetooth") || port_name.contains("debug-console");
                if skip {
                    debug!("[SerialBus] Skipping non-FC device: {}", port_name);
                }
                !skip
            })
            .collect();

        if candidate_ports.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No candidate serial ports found (all filtered out)",
            ));
        }

        debug!(
            "[SerialBus] Probing {} candidate port(s) in parallel...",
            candidate_ports.len()
        );

        // Probe all ports in parallel using tokio::spawn
        let mut probe_tasks = Vec::new();
        for port_info in candidate_ports {
            let port_name = port_info.port_name.clone();
            probe_tasks.push(tokio::spawn(async move {
                debug!("[SerialBus] Probing {} for MAVLink heartbeat...", port_name);
                match Self::probe_for_flight_controller(&port_name).await {
                    Ok(true) => {
                        info!("[SerialBus] ✓ Flight controller detected on: {}", port_name);
                        Some(port_name)
                    }
                    Ok(false) => {
                        debug!("[SerialBus] ✗ No valid FC heartbeat on: {}", port_name);
                        None
                    }
                    Err(e) => {
                        debug!("[SerialBus] ✗ Failed to probe {}: {}", port_name, e);
                        None
                    }
                }
            }));
        }

        // Wait for all probe tasks to complete
        let mut detected_fcs = Vec::new();
        for task in probe_tasks {
            if let Ok(Some(port_path)) = task.await {
                detected_fcs.push(port_path);
            }
        }

        if detected_fcs.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No flight controller found on any serial port",
            ))
        } else {
            info!(
                "[SerialBus] Detected {} flight controller(s): {:?}",
                detected_fcs.len(),
                detected_fcs
            );
            Ok(detected_fcs)
        }
    }

    /// Probe a single serial port for a valid flight controller heartbeat
    /// Returns Ok(true) if a valid FC is detected, Ok(false) if not, Err on I/O errors
    async fn probe_for_flight_controller(port_path: &str) -> io::Result<bool> {
        // Try to open the port
        let serial = match Self::new(port_path) {
            Ok(s) => s,
            Err(e) => {
                return Err(io::Error::new(
                    e.kind(),
                    format!("Failed to open {}: {}", port_path, e),
                ))
            }
        };

        let stream = serial.into_stream();
        let mut reader = mavlink::async_peek_reader::AsyncPeekReader::new(stream);

        // Wait up to 2 seconds for a heartbeat (flight controllers send at 1Hz)
        let timeout = Duration::from_secs(2);
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            match tokio::time::timeout(
                Duration::from_millis(500),
                mavlink::read_versioned_msg_async::<mavlink::common::MavMessage, _>(
                    &mut reader,
                    mavlink::ReadVersion::Any,
                ),
            )
            .await
            {
                Ok(Ok((_header, msg))) => {
                    // Check if this is a HEARTBEAT message from a flight controller
                    if let mavlink::common::MavMessage::HEARTBEAT(heartbeat) = msg {
                        debug!(
                            "[SerialBus] Received HEARTBEAT: type={:?}, autopilot={:?}",
                            heartbeat.mavtype, heartbeat.autopilot
                        );

                        // Check if this is a valid flight controller autopilot
                        let is_flight_controller = matches!(
                            heartbeat.autopilot,
                            MavAutopilot::MAV_AUTOPILOT_PX4
                                | MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA
                                | MavAutopilot::MAV_AUTOPILOT_GENERIC
                                | MavAutopilot::MAV_AUTOPILOT_GENERIC_WAYPOINTS_ONLY
                                | MavAutopilot::MAV_AUTOPILOT_GENERIC_WAYPOINTS_AND_SIMPLE_NAVIGATION_ONLY
                        );

                        if is_flight_controller {
                            return Ok(true);
                        } else {
                            warn!(
                                "[SerialBus] Device has MAVLink but not a flight controller autopilot: {:?}",
                                heartbeat.autopilot
                            );
                            return Ok(false);
                        }
                    }
                }
                Ok(Err(_)) => {
                    // Parse error, keep trying
                    continue;
                }
                Err(_) => {
                    // Timeout on this read, keep trying until overall timeout
                    continue;
                }
            }
        }

        // Timeout - no valid heartbeat received
        Ok(false)
    }
}
