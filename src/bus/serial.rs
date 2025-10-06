use std::io;
use tokio_serial::{SerialPortBuilderExt, SerialStream};

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
}
