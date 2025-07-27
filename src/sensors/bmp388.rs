use crate::bus::i2c::I2CBus;
use crate::sensors::{SensorDataFrame, SensorDriver};
use async_trait::async_trait;

compile_error!("I should not compile")


enum PressureKind {
    Static,
    Pitot,
}

pub struct Bmp388 {
    id: String,
    address: u8,
    bus_id: String,
    kind: PressureKind,
}

impl Bmp388 {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        let kind = if id.to_lowercase().starts_with("pitot") {
            PressureKind::Pitot
        } else {
            PressureKind::Static
        };
        Self { id, address, bus_id, kind }
    }
}

#[async_trait]
impl SensorDriver for Bmp388 {
    async fn init(&mut self, bus: &mut I2CBus) -> Result<(), String> {
        // Check chip ID (should be 0x50)
        let mut buf = [0u8; 1];
        bus.read_bytes(self.address, 0x00, &mut buf).await.map_err(|e| e.to_string())?;
        if buf[0] != 0x50 {
            return Err(format!("BMP388: Unexpected chip ID: 0x{:02x}", buf[0]));
        }

        // Soft reset (0x7E = 0xB6)
        bus.write_byte(self.address, 0x7E, 0xB6).await.map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Set power mode (normal), pressure and temp enabled
        // 0x1B = PWR_CTRL, 0x30 = press + temp sensor enabled
        bus.write_byte(self.address, 0x1B, 0x30).await.map_err(|e| e.to_string())?;
        // 0x1C = ODR config (optional, leave default for now)
        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String> {
        // Pressure and temperature are 24-bit unsigned
        let mut buf = [0u8; 6];
        bus.read_bytes(self.address, 0x04, &mut buf).await.map_err(|e| e.to_string())?;

        let press_raw = ((buf[2] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[0] as u32);
        let temp_raw = ((buf[5] as u32) << 16) | ((buf[4] as u32) << 8) | (buf[3] as u32);

        // Simplified scaling (not calibrated): pressure in Pa, temperature in deg C
        // Datasheet suggests this is only valid after compensation, which is complex
        let pressure = press_raw as f32 / 256.0; // crude approximation
        let temp = temp_raw as f32 / 512.0;

        let frame = match self.kind {
            PressureKind::Static => SensorDataFrame {
                accel: None,
                gyro: None,
                mag: None,
                temp: Some(temp),
                pressure_static: Some(pressure),
                pressure_pitot: None,
            },
            PressureKind::Pitot => SensorDataFrame {
                accel: None,
                gyro: None,
                mag: None,
                temp: Some(temp),
                pressure_static: None,
                pressure_pitot: Some(pressure),
            },
        };

        Ok(frame)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn bus(&self) -> &str {
        &self.bus_id
    }
}