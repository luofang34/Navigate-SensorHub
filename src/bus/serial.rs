use tokio_serial::{SerialPortBuilderExt, SerialStream};
use std::io;

/// Serial port wrapper for async communication
pub struct SerialBus {
    port: SerialStream,
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
        let port = tokio_serial::new(path, baud_rate)
            .open_native_async()?;

        Ok(Self {
            port,
            path: path.to_string(),
        })
    }

    /// Get a mutable reference to the underlying serial stream
    pub fn stream_mut(&mut self) -> &mut SerialStream {
        &mut self.port
    }

    /// Get the port path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Consume self and return the underlying SerialStream
    pub fn into_stream(self) -> SerialStream {
        self.port
    }
}
