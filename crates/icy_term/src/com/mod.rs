use std::error::Error;

#[cfg(test)]
pub mod tests;
#[cfg(test)]
pub use tests::*;

pub mod telnet;
pub use telnet::*;

pub mod raw;
pub use raw::*;

pub mod modem;
pub use modem::*;

pub mod websocket;

#[cfg(not(target_arch = "wasm32"))]
pub mod ssh;

use crate::{addresses::Terminal, ui::connect::OpenConnectionData};
pub type TermComResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub trait Com {
    fn get_name(&self) -> &'static str;
    fn default_port(&self) -> u16;

    fn send(&mut self, buf: &[u8]) -> TermComResult<usize>;
    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>>;
    fn set_terminal_type(&mut self, terminal: Terminal);

    fn disconnect(&mut self) -> TermComResult<()>;

    fn set_raw_mode(&mut self, _raw_transfer: bool) {}
}
pub struct NullConnection {}
impl Com for NullConnection {
    fn get_name(&self) -> &'static str {
        ""
    }

    fn send(&mut self, _buf: &[u8]) -> TermComResult<usize> {
        Ok(0)
    }

    fn read_data(&mut self) -> TermComResult<Option<Vec<u8>>> {
        Ok(Some(Vec::new()))
    }

    fn set_terminal_type(&mut self, _terminal: Terminal) {}

    fn disconnect(&mut self) -> TermComResult<()> {
        Ok(())
    }

    fn default_port(&self) -> u16 {
        0
    }
}
/*
#[derive(Debug, Clone, Copy)]
pub enum ConnectionError {
    ConnectionLost,
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::ConnectionLost => {
                write!(f, "connection lost")
            }
        }
    }
}

impl Error for ConnectionError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
*/
