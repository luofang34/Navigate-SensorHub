mod registry;
mod scheduler;
mod config;
mod bus;
mod sensors;
mod messages;
mod grpc_service;
mod errors;

use crate::config::load_sensor_config;
use crate::registry::init_all;
use crate::scheduler::spawn_sensor_tasks;
use crate::grpc_service::{SensorHubService, create_grpc_server};
use std::sync::Arc;
use tonic::transport::Server;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing with RUST_LOG environment variable support
    // RUST_LOG=debug for verbose, RUST_LOG=info for normal, RUST_LOG=warn for production
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    info!("[NavigateSensorHub] starting up...");

    // Load configuration from CONFIG_PATH or default
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config".to_string());
    let sensor_config_path = format!("{}/sensors.toml", config_path);
    let sensor_config = load_sensor_config(&sensor_config_path).expect("Failed to load sensor config");
    info!("[config] loaded {} sensor(s)", sensor_config.sensors.len());

    // Create gRPC service
    let grpc_service = Arc::new(SensorHubService::new());
    info!("[gRPC] Service initialized");

    // Initialize sensors and buses
    let (sensors, buses) = init_all(&sensor_config).await.expect("Initialization failed");
    info!("[registry] sensors and buses initialized");

    // Spawn sensor tasks with gRPC service
    let grpc_service_for_sensors = grpc_service.clone();
    spawn_sensor_tasks(sensors, buses, grpc_service_for_sensors, &sensor_config).await;
    info!("[main] sensor tasks launched");

    // Start gRPC server
    let host = std::env::var("GRPC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("GRPC_PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("{}:{}", host, port).parse().unwrap();
    let server = create_grpc_server(grpc_service.as_ref().clone());

    info!("[gRPC] Server starting on {}", addr);
    info!("[main] Ready to serve sensor data");

    // Run the gRPC server
    if let Err(e) = Server::builder()
        .add_service(server)
        .serve(addr)
        .await
    {
        error!("[error] gRPC server failed: {}", e);
    }
}
