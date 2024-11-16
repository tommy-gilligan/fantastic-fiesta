use embassy_rp::{
    pio::self,
    pio_programs::onewire::PioOneWire,
};

pub struct Ds18b20<'d, PIO: pio::Instance, const SM: usize> {
    wire: PioOneWire<'d, PIO, SM>,
}

impl<'d, PIO: pio::Instance, const SM: usize> Ds18b20<'d, PIO, SM> {
    pub fn new(wire: PioOneWire<'d, PIO, SM>) -> Self {
        Self { wire }
    }

    fn crc8(data: &[u8]) -> u8 {
        let mut temp;
        let mut data_byte;
        let mut crc = 0;
        for b in data {
            data_byte = *b;
            for _ in 0..8 {
                temp = (crc ^ data_byte) & 0x01;
                crc >>= 1;
                if temp != 0 {
                    crc ^= 0x8C;
                }
                data_byte >>= 1;
            }
        }
        crc
    }

    pub async fn start(&mut self) {
        self.wire.write_bytes(&[0xCC, 0x44]).await;
    }

    pub async fn temperature(&mut self) -> Result<f32, ()> {
        self.wire.write_bytes(&[0xCC, 0xBE]).await;
        let mut data = [0; 9];
        self.wire.read_bytes(&mut data).await;
        match Self::crc8(&data) == 0 {
            true => Ok(((data[1] as u32) << 8 | data[0] as u32) as f32 / 16.),
            false => Err(()),
        }
    }
}
