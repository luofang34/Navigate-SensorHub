use thiserror::Error;
use crate::bus::i2c::I2CError;

/// Comprehensive error types for the Navigate SensorHub
#[derive(Error, Debug)]
pub enum SensorError {
    #[error("I2C communication failed: {0}")]
    I2cError(#[from] I2CError),

    #[error("Serial port error: {0}")]
    SerialError(#[from] tokio_serial::Error),

    #[error("MAVLink protocol error: {0}")]
    MavlinkError(String),

    #[error("Sensor '{sensor}' initialization failed: {reason}")]
    InitError { sensor: String, reason: String },
    
    #[error("Sensor '{sensor}' read failed: {reason}")]
    ReadError { sensor: String, reason: String },
    
    #[error("Invalid sensor configuration for '{sensor}': {reason}")]
    ConfigError { sensor: String, reason: String },
    
    #[error("Sensor '{sensor}' returned invalid data: {reason}")]
    DataError { sensor: String, reason: String },
    
    #[error("Sensor '{sensor}' calibration failed: {reason}")]
    CalibrationError { sensor: String, reason: String },
    
    #[error("Unsupported sensor driver: '{driver}'")]
    UnsupportedDriver { driver: String },
    
    #[error("Bus '{bus}' not found or unavailable")]
    BusNotFound { bus: String },
    
    #[error("Bus '{bus}' communication timeout after {timeout_ms}ms")]
    BusTimeout { bus: String, timeout_ms: u64 },
    
    #[error("Sensor '{sensor}' wrong chip ID: expected {expected:#04x}, got {actual:#04x}")]
    WrongChipId { sensor: String, expected: u8, actual: u8 },
}

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration from '{path}': {source}")]
    LoadError {
        path: String,
        #[source]
        source: std::io::Error,
    },
    
    #[error("Invalid configuration format: {0}")]
    FormatError(#[from] toml::de::Error),
    
    #[error("Missing required configuration field: {field}")]
    MissingField { field: String },
    
    #[error("Invalid configuration value for '{field}': {reason}")]
    InvalidValue { field: String, reason: String },
    
    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

/// gRPC service errors  
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("gRPC server failed to start: {0}")]
    ServerStartError(#[from] tonic::transport::Error),
    
    #[error("Failed to publish sensor data: {reason}")]
    PublishError { reason: String },
    
    #[error("Invalid gRPC request: {reason}")]
    InvalidRequest { reason: String },
    
    #[error("Sensor data conversion failed: {0}")]
    ConversionError(String),
    
    #[error("No active subscribers for sensor data")]
    NoSubscribers,
}

/// Registry and initialization errors
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("Sensor registration failed: {0}")]
    RegistrationError(#[source] SensorError),
    
    #[error("Bus initialization failed: {0}")]
    BusInitError(#[from] ConfigError),
    
    #[error("Failed to create sensor driver: {0}")]
    DriverCreationError(#[source] SensorError),
    
    #[error("Resource cleanup failed: {reason}")]
    CleanupError { reason: String },
}

impl From<SensorError> for String {
    fn from(error: SensorError) -> Self {
        error.to_string()
    }
}

impl From<ConfigError> for String {
    fn from(error: ConfigError) -> Self {
        error.to_string()
    }
}

impl From<ServiceError> for String {
    fn from(error: ServiceError) -> Self {
        error.to_string()
    }
}

impl From<RegistryError> for String {
    fn from(error: RegistryError) -> Self {
        error.to_string()
    }
}

/// Result type aliases for convenience
pub type SensorResult<T> = Result<T, SensorError>;
pub type ConfigResult<T> = Result<T, ConfigError>;
pub type ServiceResult<T> = Result<T, ServiceError>;
pub type RegistryResult<T> = Result<T, RegistryError>;