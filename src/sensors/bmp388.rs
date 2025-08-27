use crate::bus::i2c::I2CBus;
use crate::sensors::{SensorDataFrame, SensorDriver};
use crate::errors::{SensorError, SensorResult};
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
    async fn init(&mut self, bus: &mut I2CBus) -> SensorResult<()> {
        // Check chip ID (should be 0x50)
        let mut buf = [0u8; 1];
        bus.read_bytes(self.address, 0x00, &mut buf).await?;
        
        if buf[0] != 0x50 {
            return Err(SensorError::WrongChipId {
                sensor: self.id.clone(),
                expected: 0x50,
                actual: buf[0],
            });
        }

        // Soft reset (0x7E = 0xB6)
        bus.write_byte(self.address, 0x7E, 0xB6).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to reset sensor: {}", e),
            })?;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Read calibration coefficients (0x31 to 0x45)
        let mut cal_buf = [0u8; 21];
        bus.read_bytes(self.address, 0x31, &mut cal_buf).await
            .map_err(|e| SensorError::CalibrationError {
                sensor: self.id.clone(),
                reason: format!("Failed to read calibration data: {}", e),
            })?;
        
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
        
        // Store raw calibration values - scaling will be done during compensation
        self.calibration = Some(Bmp388Calibration {
            t1: t1 as f64,
            t2: t2 as f64,
            t3: t3 as f64,
            
            p1: p1 as f64,
            p2: p2 as f64,
            p3: p3 as f64,
            p4: p4 as f64,
            p5: p5 as f64,
            p6: p6 as f64,
            p7: p7 as f64,
            p8: p8 as f64,
            p9: p9 as f64,
            p10: p10 as f64,
            p11: p11 as f64,
        });

        // Set oversampling configuration
        // 0x1C = OSR: [5:3]=temp_os x1 (000), [2:0]=press_os x4 (010) = 0x02
        bus.write_byte(self.address, 0x1C, 0x02).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to set oversampling: {}", e),
            })?;
        
        // Set output data rate to 50Hz
        // 0x1D = ODR: 50Hz = 0x02
        bus.write_byte(self.address, 0x1D, 0x02).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to set output data rate: {}", e),
            })?;
        
        // Set IIR filter
        // 0x1F = CONFIG: filter_coeff=1 (001) = 0x00
        bus.write_byte(self.address, 0x1F, 0x00).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to set IIR filter: {}", e),
            })?;
        
        // Enable pressure and temperature sensors and set normal mode
        // 0x1B = PWR_CTRL: [5:4]=mode=11 (normal), [1]=press_en=1, [0]=temp_en=1 = 0x33
        bus.write_byte(self.address, 0x1B, 0x33).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to enable sensors: {}", e),
            })?;
        
        // Wait for first measurement to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // Force a measurement in case normal mode isn't working
        // 0x1B = PWR_CTRL: forced mode with both sensors = 0x13
        bus.write_byte(self.address, 0x1B, 0x13).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to force measurement: {}", e),
            })?;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // Back to normal mode
        bus.write_byte(self.address, 0x1B, 0x33).await
            .map_err(|e| SensorError::InitError {
                sensor: self.id.clone(),
                reason: format!("Failed to set normal mode: {}", e),
            })?;
        
        println!("[{}] BMP388 calibration loaded", self.id);
        Ok(())
    }

    async fn read(&self, bus: &mut I2CBus) -> SensorResult<SensorDataFrame> {
        let calibration = self.calibration.as_ref()
            .ok_or_else(|| SensorError::DataError {
                sensor: self.id.clone(),
                reason: "Calibration not loaded".to_string(),
            })?;
        
        // Pressure and temperature are 24-bit unsigned
        let mut buf = [0u8; 6];
        bus.read_bytes(self.address, 0x04, &mut buf).await
            .map_err(|e| SensorError::ReadError {
                sensor: self.id.clone(),
                reason: format!("Failed to read sensor data: {}", e),
            })?;

        let press_raw = ((buf[2] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[0] as u32);
        let temp_raw = ((buf[5] as u32) << 16) | ((buf[4] as u32) << 8) | (buf[3] as u32);

        // Temperature compensation according to BMP388 datasheet (Python reference implementation)
        let partial_data1 = temp_raw as f64 - 256.0 * calibration.t1;
        let partial_data2 = calibration.t2 * partial_data1;
        let partial_data3 = partial_data1 * partial_data1;
        let partial_data4 = partial_data3 * calibration.t3;
        let partial_data5 = partial_data2 * 262144.0 + partial_data4;
        let partial_data6 = partial_data5 / 4294967296.0;
        let t_fine = partial_data6;
        // The formula outputs temperature scaled by 100, divide to get °C
        let temperature = (partial_data6 * 25.0 / 16384.0) / 100.0;
        
        // Pressure compensation according to BMP388 datasheet (Python reference implementation)
        let partial_data1 = t_fine * t_fine;
        let partial_data2 = partial_data1 / 64.0;
        let partial_data3 = partial_data2 * t_fine / 256.0;
        let partial_data4 = calibration.p8 * partial_data3 / 32.0;
        let partial_data5 = calibration.p7 * partial_data1 * 16.0;
        let partial_data6 = calibration.p6 * t_fine * 4194304.0;
        let offset = calibration.p5 * 140737488355328.0 + partial_data4 + partial_data5 + partial_data6;
        
        let partial_data2 = calibration.p4 * partial_data3 / 32.0;
        let partial_data4 = calibration.p3 * partial_data1 * 4.0;
        let partial_data5 = (calibration.p2 - 16384.0) * t_fine * 2097152.0;
        let sensitivity = (calibration.p1 - 16384.0) * 70368744177664.0 + partial_data2 + partial_data4 + partial_data5;
        
        let partial_data1 = sensitivity / 16777216.0 * press_raw as f64;
        let partial_data2 = calibration.p10 * t_fine;
        let partial_data3 = partial_data2 + 65536.0 * calibration.p9;
        let partial_data4 = partial_data3 * press_raw as f64 / 8192.0;
        let partial_data5 = partial_data4 * press_raw as f64 / 512.0;
        let partial_data6 = press_raw as f64 * press_raw as f64;
        let partial_data2 = calibration.p11 * partial_data6 / 65536.0;
        let partial_data3 = partial_data2 * press_raw as f64 / 128.0;
        let partial_data4 = offset / 4.0 + partial_data1 + partial_data5 + partial_data3;
        // The formula outputs pressure scaled by 100, divide to get Pa
        let pressure = (partial_data4 * 25.0 / 1099511627776.0) / 100.0;

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