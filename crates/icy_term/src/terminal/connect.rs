use crate::Address;
use crate::TransferProtocol;
use icy_engine::ScreenMode;
use icy_net::{modem::ModemConfiguration, telnet::TermCaps, ConnectionType};
use icy_parser_core::{BaudEmulation, MusicOption};
use std::{path::PathBuf, time::Duration};

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
    pub modem: Option<ModemConfiguration>,
    pub proxy_command: Option<String>,
    pub screen_mode: ScreenMode,
}

impl OpenConnectionData {
    pub fn from(call_adr: &Address, timeout: Duration, window_size: icy_engine::Size, modem: Option<ModemConfiguration>) -> Self {
        if timeout.as_secs() == 0 {
            panic!("Timeout must be greater than 0");
        }
        Self {
            screen_mode: call_adr.screen_mode,
            address: call_adr.address.clone(),
            connection_type: call_adr.protocol.clone(),
            baud_emulation: call_adr.baud_emulation.clone(),
            user_name: call_adr.user_name.clone(),
            password: call_adr.password.clone(),
            use_ansi_music: call_adr.ansi_music,
            proxy_command: if call_adr.proxy_command.is_empty() {
                None
            } else {
                Some(call_adr.proxy_command.clone())
            },
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

    Upload(TransferProtocol, Vec<PathBuf>),
    Download(TransferProtocol, Option<String>),
    CancelTransfer,
}
