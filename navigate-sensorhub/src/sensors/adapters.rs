/// Generic sensor adapters for embedded-hal based drivers
///
/// These adapters provide a bridge between embedded-hal sensor drivers
/// and our SensorDriver trait, enabling platform-agnostic sensor support.

#[cfg(feature = "icm426xx-driver")]
pub mod icm426xx;
