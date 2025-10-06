use navigate_sensorhub::{init_tracing, run_sensor_hub};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    init_tracing();

    // Get config path (board default or override)
    let config_path = navigate_sensorhub_berrygps_imuv4::get_config_path();

    tracing::info!(
        "[{}] Configuration path: {}",
        navigate_sensorhub_berrygps_imuv4::BOARD_NAME,
        config_path
    );

    // Run the sensor hub
    run_sensor_hub(&config_path).await
}
