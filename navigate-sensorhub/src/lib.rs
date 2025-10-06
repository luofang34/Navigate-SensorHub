// Public modules
pub mod bus;
pub mod config;
pub mod errors;
pub mod grpc_service;
pub mod hal;
pub mod messages;
pub mod registry;
pub mod scheduler;
pub mod sensors;

// Re-export commonly used types
pub use config::{load_sensor_config, SensorConfig};
pub use errors::{SensorError, SensorResult};
pub use grpc_service::{create_grpc_server, SensorHubService};
pub use registry::init_all;
pub use scheduler::spawn_sensor_tasks;

use std::sync::Arc;
use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Initialize tracing with default configuration
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();
}

/// Run the Navigate SensorHub with the given configuration path
pub async fn run_sensor_hub(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("[NavigateSensorHub] starting up...");

    // Load configuration
    let sensor_config_path = format!("{}/sensors.toml", config_path);
    let sensor_config = load_sensor_config(&sensor_config_path)?;
    info!("[config] loaded {} sensor(s)", sensor_config.sensors.len());

    // Create gRPC service BEFORE initializing sensors (MAVLink sensors need it)
    let grpc_service = Arc::new(SensorHubService::new());
    info!("[gRPC] Service initialized");

    // Initialize sensors and buses (pass gRPC service for MAVLink sensor injection)
    let (sensors, buses) = init_all(&sensor_config, grpc_service.clone()).await?;
    info!("[registry] sensors and buses initialized");

    // Spawn sensor tasks with gRPC service
    let grpc_service_for_sensors = grpc_service.clone();
    spawn_sensor_tasks(sensors, buses, grpc_service_for_sensors, &sensor_config).await;
    info!("[main] sensor tasks launched");

    // Start gRPC server
    let host = std::env::var("GRPC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("GRPC_PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("{}:{}", host, port).parse()?;
    let server = create_grpc_server(grpc_service.as_ref().clone());

    info!("[gRPC] Server starting on {}", addr);
    info!("[main] Ready to serve sensor data");

    // Run the gRPC server
    if let Err(e) = Server::builder().add_service(server).serve(addr).await {
        error!("[gRPC] Server failed: {}", e);
        return Err(Box::new(e));
    }

    Ok(())
}
