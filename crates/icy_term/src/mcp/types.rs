use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalState {
    pub cursor_position: (usize, usize),
    pub screen_size: (usize, usize),
    pub current_buffer: String,
    pub is_connected: bool,
    pub current_bbs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendTextRequest {
    pub text: String,
}

// New tool request types
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaptureScreenRequest {
    pub format: ScreenCaptureFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScreenCaptureFormat {
    Ansi,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendKeyRequest {
    pub key: String, // e.g., "Enter", "Escape", "F1", "Ctrl+C"
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunScriptRequest {
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTerminalRequest {
    pub terminal_type: String,
    pub rows: Option<usize>,
    pub columns: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFileRequest {
    pub protocol: String, // "zmodem", "xmodem", "ymodem"
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadFileRequest {
    pub protocol: String,
    pub save_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroRequest {
    pub name: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchBufferRequest {
    pub pattern: String,
    pub case_sensitive: bool,
    pub regex: bool,
}
