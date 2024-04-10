pub const SOH: u8 = 0x01;
pub const EOT: u8 = 0x04;
pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;
pub const CAN: u8 = 0x18;

pub const STX: u8 = 0x02;
pub const CPMEOF: u8 = 0x1A;

pub const DEFAULT_BLOCK_LENGTH: usize = 128;
pub const EXT_BLOCK_LENGTH: usize = 1024;
