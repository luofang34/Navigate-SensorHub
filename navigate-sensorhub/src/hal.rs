/// Hardware Abstraction Layer (HAL) for platform-agnostic sensor access
///
/// This module provides a unified interface for I2C communication that works
/// across different platforms (Linux, bare metal microcontrollers, etc.)

#[cfg(feature = "linux-hal")]
pub mod linux {
    // Re-export linux-embedded-hal types directly
    pub use linux_embedded_hal::I2cdev as I2CDevice;
    pub use linux_embedded_hal::i2cdev::linux::LinuxI2CError as I2CError;
}

#[cfg(feature = "bare-metal-hal")]
pub mod bare_metal {
    /// Placeholder for bare metal HAL implementation
    /// This will be implemented when targeting specific microcontrollers
    /// (e.g., STM32, RP2040, etc.)
    pub struct I2CDevice;

    // Implementation would depend on the specific MCU HAL crate
}

// Re-export the active platform's HAL
#[cfg(feature = "linux-hal")]
pub use linux::*;

#[cfg(feature = "bare-metal-hal")]
pub use bare_metal::*;
