use crate::crc8;
use defmt::*;
use fixed::types::extra::U4;
use fixed::FixedU16;

#[derive(Copy, Clone, PartialEq, Debug, Format)]
pub enum Resolution {
    Bits9,
    Bits10,
    Bits11,
    Bits12,
}

#[derive(Copy, Clone, PartialEq, Debug, Format)]
pub struct Configuration(u8);

impl Configuration {
    pub fn resolution(&self) -> Resolution {
        if (self.0 & 0b01000000) == 0 {
            if (self.0 & 0b00100000) == 0 {
                Resolution::Bits9
            } else {
                Resolution::Bits10
            }
        } else if (self.0 & 0b00100000) == 0 {
            Resolution::Bits11
        } else {
            Resolution::Bits12
        }
    }
}

impl From<u8> for Configuration {
    fn from(value: u8) -> Self {
        Configuration(value)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Scratchpad {
    pub temperature: FixedU16<U4>,
    pub high: u8,
    pub low: u8,
    pub configuration: Configuration,
    pub reserved: [u8; 3],
    pub crc: u8,
}

impl From<[u8; 9]> for Scratchpad {
    fn from(bytes: [u8; 9]) -> Self {
        Self {
            temperature: FixedU16::<U4>::from_le_bytes([bytes[0], bytes[1]]),
            high: bytes[2],
            low: bytes[3],
            configuration: Configuration::from(bytes[4]),
            reserved: [bytes[5], bytes[6], bytes[7]],
            crc: bytes[8],
        }
    }
}

impl Scratchpad {
    pub fn check_crc(&self) -> bool {
        let bytes = [
            self.temperature.to_le_bytes()[0],
            self.temperature.to_le_bytes()[1],
            self.high,
            self.low,
            self.configuration.0,
            self.reserved[0],
            self.reserved[1],
            self.reserved[2],
            self.crc,
        ];

        crc8(&bytes) == 0
    }
}
