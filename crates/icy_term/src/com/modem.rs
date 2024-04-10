#![allow(dead_code)]

use crate::Modem;

use super::{Com, OpenConnectionData, TermComResult};
use serial::prelude::*;
use std::io::{Read, Write};

pub struct ComModemImpl {
    modem: Modem,
    port: Box<dyn serial::SerialPort>,
}

impl ComModemImpl {
    pub fn connect(connection_data: &OpenConnectionData) -> TermComResult<Self> {
        let modem = connection_data.modem.as_ref().unwrap().clone();
        let mut port = serial::open(&modem.device)?;
        port.reconfigure(&|settings| {
            settings.set_baud_rate(serial::BaudRate::from_speed(modem.baud_rate))?;
            settings.set_char_size(modem.char_size);
            settings.set_parity(modem.parity);
            settings.set_stop_bits(modem.stop_bits);
            settings.set_flow_control(modem.flow_control);
            Ok(())
        })?;
        port.write_all(modem.init_string.as_bytes())?;
        port.write_all(b"\n")?;
        port.write_all(modem.dial_string.as_bytes())?;
        port.write_all(connection_data.address.as_bytes())?;
        port.write_all(b"\n")?;
        Ok(Self { modem, port: Box::new(port) })
    }
}

impl Com for ComModemImpl {
    fn get_name(&self) -> &'static str {
        "Modem"
    }

    fn default_port(&self) -> u16 {
        0
    }

    fn set_terminal_type(&mut self, _terminal: crate::addresses::Terminal) {}

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        let mut buf: Vec<u8> = (0..255).collect();
        match self.port.read(&mut buf[..]) {
            Ok(size) => {
                buf.truncate(size);
                Ok(Some(buf))
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn send(&mut self, buf: &[u8]) -> TermComResult<usize> {
        self.port.write_all(buf)?;
        Ok(buf.len())
    }

    fn disconnect(&mut self) -> TermComResult<()> {
        Ok(())
    }
}
