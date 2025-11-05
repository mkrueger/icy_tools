pub mod handlers;
pub mod server;
pub mod types;

use std::sync::{Arc, Mutex};

pub use server::*;
use tokio::sync::oneshot;

use crate::{
    Address,
    mcp::types::{ScreenCaptureFormat, TerminalState},
};

pub type SenderType<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

#[derive(Debug)]
pub enum McpCommand {
    Connect(String),
    Disconnect,
    SendText(String),
    SendKey(String),
    GetState(SenderType<TerminalState>),
    ListAddresses(SenderType<Vec<Address>>),
    CaptureScreen(ScreenCaptureFormat, SenderType<Vec<u8>>),
    SetTerminal {
        terminal_type: String,
        rows: Option<usize>,
        columns: Option<usize>,
    },
    UploadFile {
        protocol: String,
        file_path: String,
    },
    DownloadFile {
        protocol: String,
        save_path: String,
    },
    RunMacro {
        name: String,
        commands: Vec<String>,
    },
    SearchBuffer {
        pattern: String,
        case_sensitive: bool,
        regex: bool,
    },
    ClearScreen,
    SaveSession(String),
    LoadSession(String),
}
