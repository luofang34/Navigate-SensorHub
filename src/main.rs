mod registry;
mod scheduler;
mod config;
mod bus;
mod sensors;
mod messages;
mod grpc_service;

use crate::config::load_sensor_config;
use crate::registry::init_all;
use crate::scheduler::spawn_sensor_tasks;
use crate::grpc_service::{SensorHubService, create_grpc_server};
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() {
    println!("[NavigateSensorHub] starting up...");
    
    // Load configuration
    let sensor_config = load_sensor_config("config/sensors.toml").expect("Failed to load sensor config");
    println!("[config] loaded {} sensor(s)", sensor_config.sensors.len());
    
    // Create gRPC service
    let grpc_service = Arc::new(SensorHubService::new());
    println!("[gRPC] Service initialized");
    
    // Initialize sensors and buses
    let (sensors, buses) = init_all(&sensor_config).await.expect("Initialization failed");
    println!("[registry] sensors and buses initialized");
    
    // Spawn sensor tasks with gRPC service
    let grpc_service_for_sensors = grpc_service.clone();
    spawn_sensor_tasks(sensors, buses, grpc_service_for_sensors, &sensor_config).await;
    println!("[main] sensor tasks launched");
    
    // Start gRPC server
    let addr = "127.0.0.1:50051".parse().unwrap();
    let server = create_grpc_server(grpc_service.as_ref().clone());
    
    println!("[gRPC] Server starting on {}", addr);
    println!("[main] Ready to serve sensor data");
    
    // Run the gRPC server
    if let Err(e) = Server::builder()
        .add_service(server)
        .serve(addr)
        .await 
    {
        eprintln!("[error] gRPC server failed: {}", e);
    }
}
