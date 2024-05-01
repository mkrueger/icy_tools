use std::io::Write;

use icy_net::serial::{CharSize, FlowControl, Parity, StopBits};

use crate::TerminalResult;

#[derive(Clone, Debug, PartialEq)]
pub struct Modem {
    pub device: String,
    pub baud_rate: u32,

    pub char_size: CharSize,
    pub stop_bits: StopBits,
    pub parity: Parity,

    pub flow_control: FlowControl,

    pub init_string: String,
    pub dial_string: String,
}

impl Modem {
    pub(crate) fn write_modem_settings(&self, file: &mut std::fs::File) -> TerminalResult<()> {
        // currently unused
        file.write_all("name = \"Modem 1\"\n".to_string().as_bytes())?;

        file.write_all(format!("device = \"{}\"\n", self.device).as_bytes())?;
        file.write_all(format!("baud_rate = {}\n", self.baud_rate).as_bytes())?;
        let cs = match self.char_size {
            CharSize::Bits5 => 5,
            CharSize::Bits6 => 6,
            CharSize::Bits7 => 7,
            CharSize::Bits8 => 8,
        };
        file.write_all(format!("char_size = {cs}\n").as_bytes())?;

        let cs = match self.stop_bits {
            StopBits::One => 1,
            StopBits::Two => 2,
        };
        file.write_all(format!("stop_bits = {cs}\n").as_bytes())?;

        let cs = match self.parity {
            Parity::None => "None",
            Parity::Odd => "Odd",
            Parity::Even => "Even",
        };
        file.write_all(format!("parity = \"{cs}\"\n").as_bytes())?;

        let cs = match self.flow_control {
            FlowControl::None => "None",
            FlowControl::XonXoff => "Software",
            FlowControl::RtsCts => "Hardware",
        };
        file.write_all(format!("flow_control = \"{cs}\"\n").as_bytes())?;
        file.write_all(format!("init_string = \"{}\"\n", self.init_string).as_bytes())?;
        file.write_all(format!("dial_string = \"{}\"\n", self.dial_string).as_bytes())?;

        Ok(())
    }

    pub(crate) fn from_table(table: &toml::map::Map<String, toml::Value>) -> Modem {
        let mut result = Modem::default();
        for (k, v) in table {
            match k.as_str() {
                "device" => {
                    if let toml::Value::String(s) = v {
                        result.device = s.to_string();
                    }
                }
                "baud_rate" => {
                    if let toml::Value::Integer(i) = v {
                        result.baud_rate = *i as u32;
                    }
                }
                "char_size" => {
                    if let toml::Value::Integer(i) = v {
                        result.char_size = match i {
                            5 => CharSize::Bits5,
                            6 => CharSize::Bits6,
                            7 => CharSize::Bits7,
                            _ => CharSize::Bits8,
                        };
                    }
                }
                "stop_bits" => {
                    if let toml::Value::Integer(i) = v {
                        result.stop_bits = match i {
                            2 => StopBits::Two,
                            _ => StopBits::One,
                        };
                    }
                }
                "parity" => {
                    if let toml::Value::String(s) = v {
                        result.parity = match s.as_str() {
                            "Odd" => Parity::Odd,
                            "Even" => Parity::Even,
                            _ => Parity::None,
                        };
                    }
                }
                "flow_control" => {
                    if let toml::Value::String(s) = v {
                        result.flow_control = match s.as_str() {
                            "Software" => FlowControl::XonXoff,
                            "Hardware" => FlowControl::RtsCts,
                            _ => FlowControl::None,
                        };
                    }
                }
                "init_string" => {
                    if let toml::Value::String(s) = v {
                        result.init_string = s.to_string();
                    }
                }
                "dial_string" => {
                    if let toml::Value::String(s) = v {
                        result.dial_string = s.to_string();
                    }
                }
                _ => {}
            }
        }
        result
    }
}

impl Default for Modem {
    fn default() -> Self {
        Self {
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
