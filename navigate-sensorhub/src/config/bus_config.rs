use serde::Deserialize;
use std::fs;

/// Root structure for loading `[[bus]]` style TOML config
#[derive(Debug, Deserialize)]
pub struct BusConfig {
    #[serde(rename = "bus")]
    pub buses: Vec<BusEntry>,
}

/// One bus entry (e.g., I2C, SPI, etc.)
#[derive(Debug, Deserialize)]
pub struct BusEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String, // 'type' is a reserved word in Rust, use raw identifier
    pub path: String,
}

/// Load bus config file
pub fn load_bus_config(path: &str) -> Result<BusConfig, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let parsed: BusConfig = toml::from_str(&content).map_err(std::io::Error::other)?;
    Ok(parsed)
}
