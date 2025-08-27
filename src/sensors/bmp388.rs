use crate::bus::i2c::I2CBus;
use crate::sensors::{SensorDataFrame, SensorDriver};
use async_trait::async_trait;

enum PressureKind {
    Static,
    Pitot,
}

/// BMP388 calibration coefficients
#[derive(Debug)]
struct Bmp388Calibration {
    // Temperature compensation coefficients
    t1: f64,
    t2: f64,
    t3: f64,
    
    // Pressure compensation coefficients
    p1: f64,
    p2: f64,
    p3: f64,
    p4: f64,
    p5: f64,
    p6: f64,
    p7: f64,
    p8: f64,
    p9: f64,
    p10: f64,
    p11: f64,
}

pub struct Bmp388 {
    id: String,
    address: u8,
    bus_id: String,
    kind: PressureKind,
    calibration: Option<Bmp388Calibration>,
}

impl Bmp388 {
    pub fn new(id: String, address: u8, bus_id: String) -> Self {
        let kind = if id.to_lowercase().starts_with("pitot") {
            PressureKind::Pitot
        } else {
            PressureKind::Static
        };
        Self { id, address, bus_id, kind, calibration: None }
    }
}

#[async_trait]
impl SensorDriver for Bmp388 {
    async fn init(&mut self, bus: &mut I2CBus) -> Result<(), String> {
        // Check chip ID (should be 0x50)
        let mut buf = [0u8; 1];
        // println!("[{}] Reading chip ID from 0x{:02x}", self.id, self.address);
        bus.read_bytes(self.address, 0x00, &mut buf).await.map_err(|e| e.to_string())?;
        // println!("[{}] Got chip ID: 0x{:02x}", self.id, buf[0]);
        if buf[0] != 0x50 {
            return Err(format!("BMP388: Unexpected chip ID: 0x{:02x}", buf[0]));
        }

        // Soft reset (0x7E = 0xB6)
        bus.write_byte(self.address, 0x7E, 0xB6).await.map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Read calibration coefficients (0x31 to 0x45)
        let mut cal_buf = [0u8; 21];
        bus.read_bytes(self.address, 0x31, &mut cal_buf).await.map_err(|e| e.to_string())?;
        
        // Parse calibration data according to BMP388 datasheet
        let t1 = (cal_buf[1] as u16) << 8 | cal_buf[0] as u16;
        let t2 = (cal_buf[3] as u16) << 8 | cal_buf[2] as u16;
        let t3 = cal_buf[4] as i8;
        
        let p1 = (cal_buf[6] as i16) << 8 | cal_buf[5] as i16;
        let p2 = (cal_buf[8] as i16) << 8 | cal_buf[7] as i16;
        let p3 = cal_buf[9] as i8;
        let p4 = cal_buf[10] as i8;
        let p5 = (cal_buf[12] as u16) << 8 | cal_buf[11] as u16;
        let p6 = (cal_buf[14] as u16) << 8 | cal_buf[13] as u16;
        let p7 = cal_buf[15] as i8;
        let p8 = cal_buf[16] as i8;
        let p9 = (cal_buf[18] as i16) << 8 | cal_buf[17] as i16;
        let p10 = cal_buf[19] as i8;
        let p11 = cal_buf[20] as i8;
        
        // Convert to floating point with scale factors from datasheet
        self.calibration = Some(Bmp388Calibration {
            t1: t1 as f64 / (0.00390625f64), // 2^-8
            t2: t2 as f64 / (1073741824.0f64), // 2^30
            t3: t3 as f64 / (281474976710656.0f64), // 2^48
            
            p1: (p1 as f64 - 16384.0f64) / 1048576.0f64, // (P1 - 2^14) / 2^20
            p2: (p2 as f64 - 16384.0f64) / 536870912.0f64, // (P2 - 2^14) / 2^29
            p3: p3 as f64 / 4294967296.0f64, // P3 / 2^32
            p4: p4 as f64 / 137438953472.0f64, // P4 / 2^37
            p5: p5 as f64 / (0.125f64), // P5 / 2^-3
            p6: p6 as f64 / 64.0f64, // P6 / 2^6
            p7: p7 as f64 / 256.0f64, // P7 / 2^8
            p8: p8 as f64 / 32768.0f64, // P8 / 2^15
            p9: p9 as f64 / 281474976710656.0f64, // P9 / 2^48
            p10: p10 as f64 / 281474976710656.0f64, // P10 / 2^48
            p11: p11 as f64 / 36893488147419103232.0f64, // P11 / 2^65
        });

        // Set power mode (normal), pressure and temp enabled
        // 0x1B = PWR_CTRL, 0x30 = press + temp sensor enabled
        bus.write_byte(self.address, 0x1B, 0x30).await.map_err(|e| e.to_string())?;
        
        // Set oversampling and filter config
        // 0x1C = OSR: temp_os x2, press_os x16
        bus.write_byte(self.address, 0x1C, 0x05).await.map_err(|e| e.to_string())?;
        // 0x1F = CONFIG: IIR filter coeff 3
        bus.write_byte(self.address, 0x1F, 0x02).await.map_err(|e| e.to_string())?;
        
        println!("[{}] BMP388 calibration loaded", self.id);
        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> Result<SensorDataFrame, String> {
        let calibration = self.calibration.as_ref()
            .ok_or_else(|| "BMP388: Calibration not loaded".to_string())?;
        
        // Pressure and temperature are 24-bit unsigned
        let mut buf = [0u8; 6];
        bus.read_bytes(self.address, 0x04, &mut buf).await.map_err(|e| e.to_string())?;

        let press_raw = ((buf[2] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[0] as u32);
        let temp_raw = ((buf[5] as u32) << 16) | ((buf[4] as u32) << 8) | (buf[3] as u32);

        // Temperature compensation according to BMP388 datasheet
        let temp_comp1 = temp_raw as f64 - calibration.t1;
        let temp_comp2 = temp_comp1 * calibration.t2;
        let temperature = temp_comp2 + (temp_comp1 * temp_comp1) * calibration.t3;
        
        // Pressure compensation according to BMP388 datasheet  
        let press_comp1 = calibration.p6 * temperature;
        let press_comp2 = calibration.p7 * (temperature * temperature);
        let press_comp3 = calibration.p8 * (temperature * temperature * temperature);
        let press_offset = calibration.p5 + press_comp1 + press_comp2 + press_comp3;
        
        let press_comp4 = calibration.p1 * temperature;
        let press_comp5 = calibration.p2 * (temperature * temperature);
        let press_comp6 = calibration.p3 * (temperature * temperature * temperature);
        let press_sensitivity = (press_raw as f64) * (calibration.p4 + press_comp4 + press_comp5 + press_comp6);
        
        let press_comp7 = (press_raw as f64) * (press_raw as f64);
        let press_comp8 = calibration.p9 + calibration.p10 * temperature;
        let press_comp9 = press_comp7 * press_comp8;
        let press_comp10 = press_comp9 + (press_raw as f64) * (press_raw as f64) * (press_raw as f64) * calibration.p11;
        
        let pressure = press_offset + press_sensitivity + press_comp10;

        let frame = match self.kind {
            PressureKind::Static => SensorDataFrame {
                accel: None,
                gyro: None,
                mag: None,
                temp: Some(temperature as f32),
                pressure_static: Some(pressure as f32),
                pressure_pitot: None,
            },
            PressureKind::Pitot => SensorDataFrame {
                accel: None,
                gyro: None,
                mag: None,
                temp: Some(temperature as f32),
                pressure_static: None,
                pressure_pitot: Some(pressure as f32),
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