use icy_net::serial::{CharSize, FlowControl, Parity, StopBits};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modem {
    pub name: String,
    pub device: String,
    pub baud_rate: u32,

    pub char_size: CharSize,
    pub stop_bits: StopBits,
    pub parity: Parity,

    pub flow_control: FlowControl,

    pub init_string: String,
    pub dial_string: String,
}

impl Default for Modem {
    fn default() -> Self {
        Self {
            name: "Modem 1".to_string(),
            #[cfg(target_os = "windows")]
            device: "COM1".to_string(),
            #[cfg(not(target_os = "windows"))]
            device: "/dev/ttyS0".to_string(),
            baud_rate: 9600,
            char_size: CharSize::Bits8,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            init_string: "ATZ".to_string(),
            dial_string: "ATDT".to_string(),
        }
    }
}
