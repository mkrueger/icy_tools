use crate::{Address, Modem, TerminalResult};
use icy_engine::ansi::{BaudEmulation, MusicOption};
use icy_net::{protocol::TransferProtocolType, telnet::TermCaps, ConnectionType};
use std::{collections::VecDeque, path::PathBuf, sync::mpsc};
use web_time::{Duration, Instant};

/// A more lightweight version of `Address` that is used for the connection
///Using Addreess in `SendData` makes just the enum larger without adding any value.
#[derive(Clone, Debug)]
pub struct OpenConnectionData {
    pub address: String,
    pub connection_type: ConnectionType,
    pub user_name: String,
    pub password: String,
    pub timeout: Duration,
    pub baud_emulation: BaudEmulation,
    pub use_ansi_music: MusicOption,
    pub term_caps: TermCaps,
    pub modem: Option<Modem>,
}

impl OpenConnectionData {
    pub fn from(call_adr: &Address, timeout: Duration, window_size: icy_engine::Size, modem: Option<Modem>) -> Self {
        if timeout.as_secs() == 0 {
            panic!("Timeout must be greater than 0");
        }
        Self {
            address: call_adr.address.clone(),
            connection_type: call_adr.protocol.clone(),
            baud_emulation: call_adr.baud_emulation.clone(),
            user_name: call_adr.user_name.clone(),
            password: call_adr.password.clone(),
            use_ansi_music: call_adr.ansi_music,
            timeout,
            term_caps: TermCaps {
                window_size: (window_size.width as u16, window_size.height as u16),
                terminal: call_adr.terminal_type,
            },
            modem,
        }
    }
}

/// Data that is sent to the connection thread
#[derive(Debug)]
pub enum SendData {
    Data(Vec<u8>),
    Disconnect,
    
    SetBaudRate(u32),
    
    Upload(TransferProtocolType, Vec<PathBuf>),
    Download(TransferProtocolType),
    CancelTransfer,
}