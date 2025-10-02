use crate::messages::SensorMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Result};
use tokio_stream::Stream;
use std::pin::Pin;
use tracing::info;

// Include the generated protobuf code
pub mod sensorhub {
    tonic::include_proto!("sensorhub");
}

use sensorhub::{
    sensor_hub_server::{SensorHub, SensorHubServer},
    ImuData, MagnetometerData, BarometerData, SensorData, SensorRequest,
    SensorStatusResponse, SensorStatus, Header,
};

pub type ResponseStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

/// gRPC service implementation for sensor data streaming
#[derive(Clone)]
pub struct SensorHubService {
    // Broadcast channels for different sensor types
    imu_tx: broadcast::Sender<ImuData>,
    mag_tx: broadcast::Sender<MagnetometerData>,
    baro_tx: broadcast::Sender<BarometerData>,
    all_tx: broadcast::Sender<SensorData>,
    
    // Sensor status tracking
    sensor_stats: Arc<RwLock<HashMap<String, SensorStats>>>,
}

#[derive(Clone, Debug)]
struct SensorStats {
    is_active: bool,
    is_healthy: bool,
    frequency_hz: u32,
    messages_sent: u64,
    last_message_time_ns: u64,
    error_message: Option<String>,
}

impl Default for SensorStats {
    fn default() -> Self {
        Self {
            is_active: false,
            is_healthy: true,
            frequency_hz: 0,
            messages_sent: 0,
            last_message_time_ns: 0,
            error_message: None,
        }
    }
}

impl SensorHubService {
    pub fn new() -> Self {
        // Create broadcast channels with reasonable buffer sizes for 100Hz data
        let (imu_tx, _) = broadcast::channel(1000);
        let (mag_tx, _) = broadcast::channel(800);
        let (baro_tx, _) = broadcast::channel(800);
        let (all_tx, _) = broadcast::channel(2000);

        Self {
            imu_tx,
            mag_tx,
            baro_tx,
            all_tx,
            sensor_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Publish sensor data to appropriate streams
    pub async fn publish(&self, message: SensorMessage) -> Result<(), String> {
        let header = convert_header(message.header());
        
        match message {
            SensorMessage::Imu(imu) => {
                let imu_data = ImuData {
                    header: Some(header.clone()),
                    ax: imu.ax,
                    ay: imu.ay,
                    az: imu.az,
                    gx: imu.gx,
                    gy: imu.gy,
                    gz: imu.gz,
                };
                
                // Send to IMU-specific stream
                if let Err(_) = self.imu_tx.send(imu_data.clone()) {
                    // No active subscribers - this is fine
                }
                
                // Send to unified stream
                let sensor_data = SensorData {
                    data: Some(sensorhub::sensor_data::Data::Imu(imu_data)),
                };
                if let Err(_) = self.all_tx.send(sensor_data) {
                    // No active subscribers - this is fine
                }
                
                self.update_sensor_stats(&imu.h.sensor_id, 1).await;
            }
            
            SensorMessage::Magnetometer(mag) => {
                let mag_data = MagnetometerData {
                    header: Some(header.clone()),
                    mx: mag.mx,
                    my: mag.my,
                    mz: mag.mz,
                };
                
                if let Err(_) = self.mag_tx.send(mag_data.clone()) {
                    // No active subscribers - this is fine
                }
                
                let sensor_data = SensorData {
                    data: Some(sensorhub::sensor_data::Data::Magnetometer(mag_data)),
                };
                if let Err(_) = self.all_tx.send(sensor_data) {
                    // No active subscribers - this is fine
                }
                
                self.update_sensor_stats(&mag.h.sensor_id, 1).await;
            }
            
            SensorMessage::Barometer(baro) => {
                let baro_data = BarometerData {
                    header: Some(header.clone()),
                    pressure: baro.pressure,
                    temperature: baro.temperature,
                    altitude: baro.altitude,
                };
                
                if let Err(_) = self.baro_tx.send(baro_data.clone()) {
                    // No active subscribers - this is fine
                }
                
                let sensor_data = SensorData {
                    data: Some(sensorhub::sensor_data::Data::Barometer(baro_data)),
                };
                if let Err(_) = self.all_tx.send(sensor_data) {
                    // No active subscribers - this is fine
                }
                
                self.update_sensor_stats(&baro.h.sensor_id, 1).await;
            }
        }
        
        Ok(())
    }
    
    async fn update_sensor_stats(&self, sensor_id: &str, message_count: u64) {
        let mut stats = self.sensor_stats.write().await;
        let entry = stats.entry(sensor_id.to_string()).or_default();
        
        entry.is_active = true;
        entry.messages_sent += message_count;
        entry.last_message_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
    }
}

#[tonic::async_trait]
impl SensorHub for SensorHubService {
    type StreamIMUStream = ResponseStream<ImuData>;
    type StreamMagnetometerStream = ResponseStream<MagnetometerData>;
    type StreamBarometerStream = ResponseStream<BarometerData>;
    type StreamAllStream = ResponseStream<SensorData>;

    async fn stream_imu(
        &self,
        _request: Request<SensorRequest>,
    ) -> Result<Response<Self::StreamIMUStream>> {
        info!("[gRPC] New IMU stream client connected");

        let rx = self.imu_tx.subscribe();
        let stream = BroadcastStream::new(rx).map(|item| {
            item.map_err(|e| Status::internal(format!("Broadcast error: {}", e)))
        });
        
        Ok(Response::new(Box::pin(stream)))
    }

    async fn stream_magnetometer(
        &self,
        _request: Request<SensorRequest>,
    ) -> Result<Response<Self::StreamMagnetometerStream>> {
        info!("[gRPC] New magnetometer stream client connected");

        let rx = self.mag_tx.subscribe();
        let stream = BroadcastStream::new(rx).map(|item| {
            item.map_err(|e| Status::internal(format!("Broadcast error: {}", e)))
        });
        
        Ok(Response::new(Box::pin(stream)))
    }

    async fn stream_barometer(
        &self,
        _request: Request<SensorRequest>,
    ) -> Result<Response<Self::StreamBarometerStream>> {
        info!("[gRPC] New barometer stream client connected");

        let rx = self.baro_tx.subscribe();
        let stream = BroadcastStream::new(rx).map(|item| {
            item.map_err(|e| Status::internal(format!("Broadcast error: {}", e)))
        });
        
        Ok(Response::new(Box::pin(stream)))
    }

    async fn stream_all(
        &self,
        _request: Request<SensorRequest>,
    ) -> Result<Response<Self::StreamAllStream>> {
        info!("[gRPC] New unified stream client connected");

        let rx = self.all_tx.subscribe();
        let stream = BroadcastStream::new(rx).map(|item| {
            item.map_err(|e| Status::internal(format!("Broadcast error: {}", e)))
        });
        
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_sensor_status(
        &self,
        _request: Request<SensorRequest>,
    ) -> Result<Response<SensorStatusResponse>> {
        let stats = self.sensor_stats.read().await;
        let sensor_statuses: Vec<SensorStatus> = stats
            .iter()
            .map(|(sensor_id, stats)| SensorStatus {
                sensor_id: sensor_id.clone(),
                is_active: stats.is_active,
                is_healthy: stats.is_healthy,
                frequency_hz: stats.frequency_hz,
                messages_sent: stats.messages_sent,
                last_message_time_ns: stats.last_message_time_ns,
                error_message: stats.error_message.clone(),
            })
            .collect();

        Ok(Response::new(SensorStatusResponse {
            sensors: sensor_statuses,
        }))
    }
}

/// Convert internal message header to protobuf header
fn convert_header(header: &crate::messages::Header) -> Header {
    Header {
        device_id: header.device_id.clone(),
        sensor_id: header.sensor_id.clone(),
        frame_id: header.frame_id.clone(),
        seq: header.seq,
        t_utc_ns: header.t_utc_ns,
        t_mono_ns: header.t_mono_ns,
        pps_locked: header.pps_locked,
        ptp_locked: header.ptp_locked,
        clock_err_ppb: header.clock_err_ppb,
        sigma_t_ns: header.sigma_t_ns,
        schema_v: header.schema_v as u32,
    }
}

/// Create and configure gRPC server
pub fn create_grpc_server(service: SensorHubService) -> SensorHubServer<SensorHubService> {
    SensorHubServer::new(service)
        .max_encoding_message_size(1024 * 1024)  // 1MB max message size
        .max_decoding_message_size(1024 * 1024)
}