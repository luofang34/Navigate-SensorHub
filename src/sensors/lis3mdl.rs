use super::{SensorDataFrame, SensorDriver};
use crate::sensors::SensorFactory;
use crate::bus::i2c::I2CBus;
use async_trait::async_trait;

// Register addresses for the LIS3MDL
const WHO_AM_I: u8 = 0x0F;
const CTRL_REG1: u8 = 0x20;
const CTRL_REG2: u8 = 0x21;
const CTRL_REG3: u8 = 0x22;
const CTRL_REG4: u8 = 0x23;
const OUT_X_L: u8 = 0x28;

// Sensitivity for +/- 4 gauss full scale
const SENSITIVITY_4GAUSS: f32 = 0.00014; // Tesla per LSB

pub struct Lis3mdl {
    id: String,
    address: u8,
    bus_id: String,
}

impl Lis3mdl {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        Self { id, address, bus_id }
    }
}

#[async_trait]
impl SensorDriver for Lis3mdl {
    async fn init(&mut self, bus: &mut I2CBus) -> Result<(), String> {
        // Verify device identity
        let mut who_am_i_buf = [0u8; 1];
        bus.read_bytes(self.address, WHO_AM_I, &mut who_am_i_buf).await.map_err(|e| e.to_string())?;
        if who_am_i_buf[0] != 0x3D {
            return Err(format!("LIS3MDL WHO_AM_I check failed. Expected 0x3D, got {:#04x}", who_am_i_buf[0]));
        }

        // Configure magnetometer:
        // CTRL_REG1: Temp sensor disabled, medium-performance mode, 80 Hz ODR
        bus.write_byte(self.address, CTRL_REG1, 0b01011100).await.map_err(|e| e.to_string())?;
        // CTRL_REG2: +/- 4 gauss full scale
        bus.write_byte(self.address, CTRL_REG2, 0b00000000).await.map_err(|e| e.to_string())?;
        // CTRL_REG3: Continuous-conversion mode
        bus.write_byte(self.address, CTRL_REG3, 0b00000000).await.map_err(|e| e.to_string())?;
        // CTRL_REG4: Z-axis medium-performance mode
        bus.write_byte(self.address, CTRL_REG4, 0b00000100).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String> {
        let mut frame = SensorDataFrame::default();

        // Read magnetometer data
        let mut mag_buf = [0u8; 6];
        bus.read_bytes(self.address, OUT_X_L, &mut mag_buf).await.map_err(|e| e.to_string())?;

        let mag_raw = [
            i16::from_le_bytes([mag_buf[0], mag_buf[1]]),
            i16::from_le_bytes([mag_buf[2], mag_buf[3]]),
            i16::from_le_bytes([mag_buf[4], mag_buf[5]]),
        ];

        frame.mag = Some([
            mag_raw[0] as f32 * SENSITIVITY_4GAUSS,
            mag_raw[1] as f32 * SENSITIVITY_4GAUSS,
            mag_raw[2] as f32 * SENSITIVITY_4GAUSS,
        ]);

        Ok(frame)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn bus(&self) -> &str {
        &self.bus_id
    }
}

pub struct Lis3mdlFactory;

impl SensorFactory for Lis3mdlFactory {
    fn create(&self, id: String, address: u8, bus_id: String) -> Box<dyn SensorDriver + Send> {
        Box::new(Lis3mdl::new(id, address, bus_id))
    }

    fn name(&self) -> &'static str {
        "lis3mdl"
    }
}

pub static LIS3MDL_FACTORY: Lis3mdlFactory = Lis3mdlFactory;
