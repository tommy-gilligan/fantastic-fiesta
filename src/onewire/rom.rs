use crate::crc8;
use defmt::*;

#[derive(Debug, PartialEq, Format, Copy, Clone)]
pub enum SearchErr {
    NoReply,
    DisconnectDuringSearch,
}

#[derive(Copy, Clone, PartialEq, Debug, Format)]
pub struct RegistrationNumber {
    pub crc: u8,
    pub serial: [u8; 6],
    pub family_code: u8,
}

impl RegistrationNumber {
    pub fn check_crc(&self) -> bool {
        let bytes = [
            self.family_code,
            self.serial[0],
            self.serial[1],
            self.serial[2],
            self.serial[3],
            self.serial[4],
            self.serial[5],
            self.crc,
        ];
        crc8(&bytes) == 0
    }
}

impl From<u64> for RegistrationNumber {
    fn from(value: u64) -> Self {
        let bytes = value.to_le_bytes();
        Self {
            family_code: bytes[0],
            serial: [bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6]],
            crc: bytes[7],
        }
    }
}

impl From<RegistrationNumber> for u64 {
    fn from(value: RegistrationNumber) -> Self {
        let bytes = [
            value.family_code,
            value.serial[0],
            value.serial[1],
            value.serial[2],
            value.serial[3],
            value.serial[4],
            value.serial[5],
            value.crc,
        ];
        u64::from_le_bytes(bytes)
    }
}
