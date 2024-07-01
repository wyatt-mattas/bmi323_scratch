use crate::{
    interface::{I2cInterface, ReadData, SpiInterface, WriteData},
    types::{AccelerometerRange, GyroscopeRange, Sensor3DData, Sensor3DDataScaled, SensorType},
    AccelConfig, Bmi323, Error, GyroConfig, Register,
};
use embedded_hal::delay::DelayNs;

impl<I2C, D> Bmi323<I2cInterface<I2C>, D>
where
    D: DelayNs,
{
    pub fn new_with_i2c(i2c: I2C, address: u8, delay: D) -> Self {
        Bmi323 {
            iface: I2cInterface { i2c, address },
            delay,
            accel_range: AccelerometerRange::default(),
            gyro_range: GyroscopeRange::default(),
        }
    }
}

impl<SPI, D> Bmi323<SpiInterface<SPI>, D>
where
    D: DelayNs,
{
    pub fn new_with_spi(spi: SPI, delay: D) -> Self {
        Bmi323 {
            iface: SpiInterface { spi },
            delay,
            accel_range: AccelerometerRange::default(),
            gyro_range: GyroscopeRange::default(),
        }
    }
}

impl<DI, D, E> Bmi323<DI, D>
where
    DI: ReadData<Error = Error<E>> + WriteData<Error = Error<E>>,
    D: DelayNs,
{
    /// Create a new BMI323 device instance
    ///
    /// # Arguments
    ///
    /// * `iface` - The communication interface (I2C or SPI)
    /// * `delay` - A delay provider
    pub fn new(iface: DI, delay: D) -> Self {
        Self {
            iface,
            delay,
            accel_range: AccelerometerRange::default(),
            gyro_range: GyroscopeRange::default(),
        }
    }

    /// Initialize the device
    pub fn init(&mut self) -> Result<(), Error<E>> {
        self.write_register_16bit(Register::CMD, Register::CMD_SOFT_RESET)?;
        self.delay.delay_us(2000);

        let result = self.read_register(0x01)?;
        if result != 0 {
            return Err(Error::InvalidDevice);
        }

        let result = self.read_register(Register::CHIPID)?;
        if result != Register::BMI323_CHIP_ID {
            return Err(Error::InvalidDevice);
        }

        Ok(())
    }

    /// Set the accelerometer configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The accelerometer configuration
    pub fn set_accel_config(&mut self, config: AccelConfig) -> Result<(), Error<E>> {
        let reg_data = self.config_to_reg_data(config);
        self.write_register_16bit(Register::ACC_CONF, reg_data)?;
        self.accel_range = config.range;
        Ok(())
    }

    /// Set the gyroscope configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The gyroscope configuration
    pub fn set_gyro_config(&mut self, config: GyroConfig) -> Result<(), Error<E>> {
        let reg_data = self.config_to_reg_data(config);
        self.write_register_16bit(Register::GYR_CONF, reg_data)?;
        self.gyro_range = config.range;
        Ok(())
    }

    fn config_to_reg_data<T>(&self, config: T) -> u16
    where
        T: Into<u16> + Copy,
    {
        let config: u16 = config.into();
        config
    }

    fn read_sensor_data(&mut self, sensor_type: SensorType) -> Result<Sensor3DData, Error<E>> {
        let (base_reg, data_size) = match sensor_type {
            SensorType::Accelerometer => (Register::ACC_DATA_X, 21),
            SensorType::Gyroscope => (Register::GYR_DATA_X, 15),
        };

        let mut data = [0u8; 21]; // Use the larger size
        data[0] = base_reg;
        let sensor_data = self.read_data(&mut data[0..data_size])?;

        Ok(Sensor3DData {
            x: i16::from_le_bytes([sensor_data[0], sensor_data[1]]),
            y: i16::from_le_bytes([sensor_data[2], sensor_data[3]]),
            z: i16::from_le_bytes([sensor_data[4], sensor_data[5]]),
        })
    }

    pub fn read_accel_data(&mut self) -> Result<Sensor3DData, Error<E>> {
        self.read_sensor_data(SensorType::Accelerometer)
    }

    pub fn read_gyro_data(&mut self) -> Result<Sensor3DData, Error<E>> {
        self.read_sensor_data(SensorType::Gyroscope)
    }

    pub fn read_accel_data_scaled(&mut self) -> Result<Sensor3DDataScaled, Error<E>> {
        let raw_data = self.read_accel_data()?;
        Ok(raw_data.to_mps2(self.accel_range.to_g())) // Assuming 16-bit width
    }

    pub fn read_gyro_data_scaled(&mut self) -> Result<Sensor3DDataScaled, Error<E>> {
        let raw_data = self.read_gyro_data()?;
        Ok(raw_data.to_dps(self.gyro_range.to_dps())) // Assuming 16-bit width
    }

    /*
    pub fn read_temperature(&mut self) -> Result<f32, Error<E>> {
        let mut data = [0u8; 2];
        self.read_data(&mut data)?;
        let raw_temp = i16::from_le_bytes([data[0], data[1]]);
        Ok((raw_temp as f32 / 512.0) + 23.0)
    }

    pub fn read_sensor_time(&mut self) -> Result<u32, Error<E>> {
        let mut data = [0u8; 3];
        self.read_data(&mut data)?;
        Ok(u32::from_le_bytes([data[0], data[1], data[2], 0]))
        }

    fn write_register(&mut self, reg: u8, value: u8) -> Result<(), Error<E>> {
        self.iface.write_register(reg, value)
        }*/

    fn write_register_16bit(&mut self, reg: u8, value: u16) -> Result<(), Error<E>> {
        let bytes = value.to_le_bytes();
        self.iface.write_data(&[reg, bytes[0], bytes[1]])
    }

    fn read_register(&mut self, reg: u8) -> Result<u8, Error<E>> {
        self.iface.read_register(reg)
    }

    fn read_data<'a>(&mut self, data: &'a mut [u8]) -> Result<&'a [u8], Error<E>> {
        self.iface.read_data(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_sensor3d_data(data: &[u8]) -> Sensor3DData {
        Sensor3DData {
            x: i16::from_le_bytes([data[0], data[1]]),
            y: i16::from_le_bytes([data[2], data[3]]),
            z: i16::from_le_bytes([data[4], data[5]]),
        }
    }

    mod sensor3d_data {
        use super::*;

        #[test]
        fn can_decode_positive_array() {
            let result = get_sensor3d_data(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
            assert_eq!(
                result,
                Sensor3DData {
                    x: 0x0201,
                    y: 0x0403,
                    z: 0x0605
                }
            );
        }

        #[test]
        fn can_decode_negative_array() {
            let result = get_sensor3d_data(&[0x0B, 0x86, 0x0B, 0x86, 0x0B, 0x86]);
            assert_eq!(
                result,
                Sensor3DData {
                    x: -31221,
                    y: -31221,
                    z: -31221
                }
            );
        }
    }
}
