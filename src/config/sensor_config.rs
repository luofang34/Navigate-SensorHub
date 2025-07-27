use serde::Deserialize;
use std::fs;

/// Root configuration struct expecting `[[sensor]]` TOML array format
#[derive(Debug, Deserialize)]
pub struct SensorConfig {
    #[serde(rename = "sensor")]
    pub sensors: Vec<SensorEntry>,
}

/// One sensor entry, matching each `[[sensor]]` section
#[derive(Debug, Deserialize)]
pub struct SensorEntry {
    pub id: String,
    pub driver: String,
    pub bus: String,
    pub address: u8,
    pub frequency: Option<u32>,
}

/// Loads config from TOML file
pub fn load_sensor_config(path: &str) -> Result<SensorConfig, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let parsed: SensorConfig = toml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(parsed)
}
