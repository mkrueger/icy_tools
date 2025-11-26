pub mod handlers;
pub mod server;
pub mod types;

use parking_lot::Mutex;
use std::sync::Arc;

pub use server::*;
use tokio::sync::oneshot;

use crate::{
    Address,
    mcp::types::{ScreenCaptureFormat, TerminalState},
};

pub type SenderType<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

/// Result type for script execution: Ok(output) or Err(error_message)
pub type ScriptResult = Result<String, String>;

#[derive(Debug)]
pub enum McpCommand {
    Connect(String),
    Disconnect,
    SendText(String),
    SendKey(String),
    GetState(SenderType<TerminalState>),
    ListAddresses(SenderType<Vec<Address>>),
    CaptureScreen(ScreenCaptureFormat, SenderType<Vec<u8>>),

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
    /// Run a Lua script with optional response channel for the result
    RunScript(String, Option<SenderType<ScriptResult>>),
}
