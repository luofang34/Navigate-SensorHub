use serde::{Deserialize, Serialize};

/// Header metadata common to all sensor messages
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Header {
    /// Unique device identifier
    pub device_id: String,
    /// Sensor type identifier (e.g., "imu0", "baro1", "mag0")
    pub sensor_id: String,
    /// Reference frame identifier
    pub frame_id: String,
    /// Sequence number for message ordering
    pub seq: u64,
    /// UTC timestamp in nanoseconds
    pub t_utc_ns: u64,
    /// CLOCK_MONOTONIC_RAW timestamp in nanoseconds
    pub t_mono_ns: u64,
    /// PPS signal lock status
    pub pps_locked: bool,
    /// PTP synchronization status
    pub ptp_locked: bool,
    /// Clock frequency error in parts per billion
    pub clock_err_ppb: i32,
    /// Timing uncertainty in nanoseconds
    pub sigma_t_ns: u32,
    /// Message schema version for evolution
    pub schema_v: u16,
}

impl Header {
    /// Create a new header with current timestamps
    pub fn new(device_id: String, sensor_id: String, frame_id: String, seq: u64) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now_utc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        
        // Get monotonic time using tokio's Instant
        let mono_start = std::time::Instant::now();
        let t_mono_ns = mono_start.elapsed().as_nanos() as u64;
        
        Self {
            device_id,
            sensor_id,
            frame_id,
            seq,
            t_utc_ns: now_utc,
            t_mono_ns,
            pps_locked: false, // TODO: Implement PPS detection
            ptp_locked: false, // TODO: Implement PTP detection
            clock_err_ppb: 0,  // TODO: Implement clock error measurement
            sigma_t_ns: 1000,  // Default 1μs uncertainty
            schema_v: 1,
        }
    }
}

/// IMU sensor data (accelerometer + gyroscope)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImuMessage {
    pub h: Header,
    /// Acceleration X-axis (m/s²)
    pub ax: f32,
    /// Acceleration Y-axis (m/s²)
    pub ay: f32,
    /// Acceleration Z-axis (m/s²)
    pub az: f32,
    /// Angular velocity X-axis (rad/s)
    pub gx: f32,
    /// Angular velocity Y-axis (rad/s)
    pub gy: f32,
    /// Angular velocity Z-axis (rad/s)
    pub gz: f32,
}

/// Magnetometer sensor data
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MagnetometerMessage {
    pub h: Header,
    /// Magnetic field X-axis (μT)
    pub mx: f32,
    /// Magnetic field Y-axis (μT)
    pub my: f32,
    /// Magnetic field Z-axis (μT)
    pub mz: f32,
}

/// Barometer sensor data
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BarometerMessage {
    pub h: Header,
    /// Atmospheric pressure (Pa)
    pub pressure: f32,
    /// Temperature (°C)
    pub temperature: f32,
    /// Calculated altitude (m) - based on standard atmosphere
    pub altitude: f32,
}

/// Unified sensor message enum for different sensor types
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SensorMessage {
    Imu(ImuMessage),
    Magnetometer(MagnetometerMessage),
    Barometer(BarometerMessage),
}

impl SensorMessage {
    /// Get the header from any sensor message
    pub fn header(&self) -> &Header {
        match self {
            SensorMessage::Imu(msg) => &msg.h,
            SensorMessage::Magnetometer(msg) => &msg.h,
            SensorMessage::Barometer(msg) => &msg.h,
        }
    }
    
    /// Get the sensor ID from any sensor message
    pub fn sensor_id(&self) -> &str {
        &self.header().sensor_id
    }
    
    
    /// Serialize to JSON for debugging
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_creation() {
        let header = Header::new(
            "test_device".to_string(),
            "imu0".to_string(),
            "base_link".to_string(),
            42,
        );
        
        assert_eq!(header.device_id, "test_device");
        assert_eq!(header.sensor_id, "imu0");
        assert_eq!(header.seq, 42);
        assert_eq!(header.schema_v, 1);
        assert!(header.t_utc_ns > 0);
    }

    #[test]
    fn test_imu_message_serialization() {
        let header = Header::new(
            "test".to_string(),
            "imu0".to_string(),
            "base_link".to_string(),
            1,
        );
        
        let imu_msg = ImuMessage {
            h: header,
            ax: 1.0, ay: 2.0, az: 9.81,
            gx: 0.1, gy: 0.2, gz: 0.3,
        };
        
        let sensor_msg = SensorMessage::Imu(imu_msg.clone());

        // Test JSON serialization round-trip
        let json = sensor_msg.to_json().unwrap();
        assert!(json.contains("imu0"));
        assert!(json.contains("9.81"));

        // Test serde round-trip via JSON
        let decoded: SensorMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SensorMessage::Imu(decoded_imu) => {
                assert_eq!(decoded_imu.ax, 1.0);
                assert_eq!(decoded_imu.az, 9.81);
                assert_eq!(decoded_imu.h.sensor_id, "imu0");
            }
            _ => panic!("Wrong message type"),
        }
    }
}