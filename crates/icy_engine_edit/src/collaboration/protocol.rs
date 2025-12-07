//! Moebius-compatible WebSocket protocol types.
//!
//! This module defines all message types used in the collaboration protocol.
//! The protocol is designed to be fully compatible with Moebius while supporting
//! extensions through protocol versioning.

use serde::{Deserialize, Serialize};

/// Action codes matching Moebius protocol.
/// These are the message types exchanged between client and server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ActionCode {
    /// Server confirms connection, sends document state
    Connected = 0,
    /// Server refuses connection (wrong password, etc.)
    Refused = 1,
    /// A user joined the session
    Join = 2,
    /// A user left the session
    Leave = 3,
    /// Cursor position update
    Cursor = 4,
    /// Selection update
    Selection = 5,
    /// Resize canvas columns
    ResizeColumns = 6,
    /// Resize canvas rows
    ResizeRows = 7,
    /// Draw a single character cell
    Draw = 8,
    /// Preview of a character cell (temporary, not saved)
    DrawPreview = 9,
    /// Chat message
    Chat = 10,
    /// Server status update
    Status = 11,
    /// Undo action
    Undo = 12,
    /// Redo action
    Redo = 13,
    /// Paste image (reference data)
    PasteImage = 14,
    /// Paste block of cells
    Paste = 15,
    /// Connection request from client
    Connect = 16,
    /// Ping/keepalive
    Ping = 17,
    /// Set use9px (9-pixel font mode)
    SetUse9px = 18,
    /// Set ice colors mode
    SetIceColors = 19,
    /// Set font
    SetFont = 20,
    /// Set background color for canvas
    SetBackground = 21,
}

/// Protocol version for feature negotiation.
/// Moebius ignores unknown fields, so this is backwards-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtocolVersion {
    /// Moebius-compatible, single layer only
    #[default]
    V1 = 1,
    /// Extended with layer support
    V2 = 2,
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u8::deserialize(deserializer)?;
        match v {
            2 => Ok(ProtocolVersion::V2),
            _ => Ok(ProtocolVersion::V1),
        }
    }
}

/// A single character cell in the document.
/// Matches Moebius block format for compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Block {
    /// Unicode code point for the character
    pub code: u32,
    /// Foreground color index (0-15 for standard, extended for truecolor)
    pub fg: u8,
    /// Background color index (0-7 for standard, extended for ice colors)
    pub bg: u8,
}

/// RGB color value for truecolor support.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Extended block with truecolor support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedBlock {
    #[serde(flatten)]
    pub base: Block,
    /// Optional truecolor foreground
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg_rgb: Option<RgbColor>,
    /// Optional truecolor background
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_rgb: Option<RgbColor>,
}

/// User information for session participants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique user ID assigned by server
    pub id: u32,
    /// Display name (nickname)
    pub nick: String,
    /// User's cursor column position
    #[serde(default)]
    pub col: i32,
    /// User's cursor row position
    #[serde(default)]
    pub row: i32,
    /// Whether user is currently selecting
    #[serde(default)]
    pub selecting: bool,
    /// Selection start column (if selecting)
    #[serde(default)]
    pub selection_col: i32,
    /// Selection start row (if selecting)
    #[serde(default)]
    pub selection_row: i32,
}

/// Chat message in session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// User nickname who sent the message
    pub nick: String,
    /// Message text content
    pub text: String,
    /// Timestamp (optional, server-assigned)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
}

/// Server status for display.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStatus {
    /// Status text to display
    pub text: String,
}

// ============================================================================
// Client -> Server Messages
// ============================================================================

/// Connect request sent by client to join a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    /// Action code (always Connect = 16)
    pub action: u8,
    /// Session password (can be empty string)
    pub pass: String,
    /// User's nickname
    pub nick: String,
    /// Protocol version (optional, Moebius ignores this)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<ProtocolVersion>,
}

impl ConnectRequest {
    pub fn new(nick: String, password: String) -> Self {
        Self {
            action: ActionCode::Connect as u8,
            pass: password,
            nick,
            protocol_version: Some(ProtocolVersion::V2),
        }
    }

    pub fn moebius_compatible(nick: String, password: String) -> Self {
        Self {
            action: ActionCode::Connect as u8,
            pass: password,
            nick,
            protocol_version: None,
        }
    }
}

/// Cursor position update from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorMessage {
    pub action: u8,
    pub col: i32,
    pub row: i32,
}

impl CursorMessage {
    pub fn new(col: i32, row: i32) -> Self {
        Self {
            action: ActionCode::Cursor as u8,
            col,
            row,
        }
    }
}

/// Selection update from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionMessage {
    pub action: u8,
    pub selecting: bool,
    pub col: i32,
    pub row: i32,
}

impl SelectionMessage {
    pub fn new(selecting: bool, col: i32, row: i32) -> Self {
        Self {
            action: ActionCode::Selection as u8,
            selecting,
            col,
            row,
        }
    }
}

/// Draw a single character cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawMessage {
    pub action: u8,
    /// Column position (x)
    pub col: i32,
    /// Row position (y)
    pub row: i32,
    /// The character block to draw
    pub block: Block,
    /// Layer index (V2 extension, Moebius ignores)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
}

impl DrawMessage {
    pub fn new(col: i32, row: i32, block: Block) -> Self {
        Self {
            action: ActionCode::Draw as u8,
            col,
            row,
            block,
            layer: None,
        }
    }

    pub fn with_layer(col: i32, row: i32, block: Block, layer: usize) -> Self {
        Self {
            action: ActionCode::Draw as u8,
            col,
            row,
            block,
            layer: Some(layer),
        }
    }
}

/// Preview draw (temporary, not saved).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawPreviewMessage {
    pub action: u8,
    pub col: i32,
    pub row: i32,
    pub block: Block,
}

impl DrawPreviewMessage {
    pub fn new(col: i32, row: i32, block: Block) -> Self {
        Self {
            action: ActionCode::DrawPreview as u8,
            col,
            row,
            block,
        }
    }
}

/// Chat message from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendMessage {
    pub action: u8,
    pub text: String,
}

impl ChatSendMessage {
    pub fn new(text: String) -> Self {
        Self {
            action: ActionCode::Chat as u8,
            text,
        }
    }
}

/// Resize canvas columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeColumnsMessage {
    pub action: u8,
    pub columns: u32,
}

impl ResizeColumnsMessage {
    pub fn new(columns: u32) -> Self {
        Self {
            action: ActionCode::ResizeColumns as u8,
            columns,
        }
    }
}

/// Resize canvas rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeRowsMessage {
    pub action: u8,
    pub rows: u32,
}

impl ResizeRowsMessage {
    pub fn new(rows: u32) -> Self {
        Self {
            action: ActionCode::ResizeRows as u8,
            rows,
        }
    }
}

/// Paste block of cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteMessage {
    pub action: u8,
    /// Compressed data (RLE format)
    pub data: String,
    /// Target column
    pub col: i32,
    /// Target row
    pub row: i32,
    /// Width of pasted area
    pub columns: u32,
    /// Height of pasted area
    pub rows: u32,
    /// Layer index (V2 extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
}

/// Ping message for keepalive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    pub action: u8,
}

impl Default for PingMessage {
    fn default() -> Self {
        Self {
            action: ActionCode::Ping as u8,
        }
    }
}

/// Set 9-pixel font mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetUse9pxMessage {
    pub action: u8,
    pub value: bool,
}

impl SetUse9pxMessage {
    pub fn new(value: bool) -> Self {
        Self {
            action: ActionCode::SetUse9px as u8,
            value,
        }
    }
}

/// Set ice colors mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetIceColorsMessage {
    pub action: u8,
    pub value: bool,
}

impl SetIceColorsMessage {
    pub fn new(value: bool) -> Self {
        Self {
            action: ActionCode::SetIceColors as u8,
            value,
        }
    }
}

/// Set font.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFontMessage {
    pub action: u8,
    pub font: String,
}

impl SetFontMessage {
    pub fn new(font: String) -> Self {
        Self {
            action: ActionCode::SetFont as u8,
            font,
        }
    }
}

/// Set background color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBackgroundMessage {
    pub action: u8,
    pub value: u32,
}

impl SetBackgroundMessage {
    pub fn new(value: u32) -> Self {
        Self {
            action: ActionCode::SetBackground as u8,
            value,
        }
    }
}

// ============================================================================
// Server -> Client Messages
// ============================================================================

/// Connection accepted response with document state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedResponse {
    /// Action code (Connected = 0)
    pub action: u8,
    /// Assigned user ID
    pub id: u32,
    /// Compressed document data (RLE format)
    pub doc: String,
    /// List of users currently in session
    pub users: Vec<User>,
    /// Chat history
    pub chat_history: Vec<ChatMessage>,
    /// Current server status
    #[serde(default)]
    pub status: ServerStatus,
    /// Protocol version supported by server (V2 extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<ProtocolVersion>,
    /// Number of columns in document
    #[serde(default = "default_columns")]
    pub columns: u32,
    /// Number of rows in document
    #[serde(default = "default_rows")]
    pub rows: u32,
    /// Use 9-pixel font
    #[serde(default)]
    pub use_9px: bool,
    /// Ice colors enabled
    #[serde(default)]
    pub ice_colors: bool,
    /// Font name
    #[serde(default)]
    pub font: String,
}

fn default_columns() -> u32 {
    80
}

fn default_rows() -> u32 {
    25
}

/// Connection refused response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusedResponse {
    pub action: u8,
    /// Reason for refusal
    #[serde(default)]
    pub reason: String,
}

/// User joined notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinMessage {
    pub action: u8,
    pub user: User,
}

/// User left notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveMessage {
    pub action: u8,
    pub id: u32,
}

/// Chat message broadcast from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBroadcastMessage {
    pub action: u8,
    pub nick: String,
    pub text: String,
}

/// Status update from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub action: u8,
    pub text: String,
}

// ============================================================================
// Generic Message Parsing
// ============================================================================

/// Incoming message that can be any action type.
/// Use this for initial parsing to determine the action code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub action: u8,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

impl IncomingMessage {
    /// Get the action code for this message.
    pub fn action_code(&self) -> Option<ActionCode> {
        match self.action {
            0 => Some(ActionCode::Connected),
            1 => Some(ActionCode::Refused),
            2 => Some(ActionCode::Join),
            3 => Some(ActionCode::Leave),
            4 => Some(ActionCode::Cursor),
            5 => Some(ActionCode::Selection),
            6 => Some(ActionCode::ResizeColumns),
            7 => Some(ActionCode::ResizeRows),
            8 => Some(ActionCode::Draw),
            9 => Some(ActionCode::DrawPreview),
            10 => Some(ActionCode::Chat),
            11 => Some(ActionCode::Status),
            12 => Some(ActionCode::Undo),
            13 => Some(ActionCode::Redo),
            14 => Some(ActionCode::PasteImage),
            15 => Some(ActionCode::Paste),
            16 => Some(ActionCode::Connect),
            17 => Some(ActionCode::Ping),
            18 => Some(ActionCode::SetUse9px),
            19 => Some(ActionCode::SetIceColors),
            20 => Some(ActionCode::SetFont),
            21 => Some(ActionCode::SetBackground),
            _ => None,
        }
    }
}

/// Parsed server message.
#[derive(Debug, Clone)]
pub enum ServerMessage {
    Connected(ConnectedResponse),
    Refused(RefusedResponse),
    Join(JoinMessage),
    Leave(LeaveMessage),
    Cursor { id: u32, col: i32, row: i32 },
    Selection { id: u32, selecting: bool, col: i32, row: i32 },
    ResizeColumns { columns: u32 },
    ResizeRows { rows: u32 },
    Draw(DrawMessage),
    DrawPreview(DrawPreviewMessage),
    Chat(ChatBroadcastMessage),
    Status(StatusMessage),
    Paste(PasteMessage),
    SetUse9px { value: bool },
    SetIceColors { value: bool },
    SetFont { font: String },
    SetBackground { value: u32 },
    Ping,
    Unknown(u8),
}

/// Parse a JSON string into a ServerMessage.
pub fn parse_server_message(json: &str) -> Result<ServerMessage, serde_json::Error> {
    let incoming: IncomingMessage = serde_json::from_str(json)?;

    match incoming.action {
        0 => {
            let resp: ConnectedResponse = serde_json::from_str(json)?;
            Ok(ServerMessage::Connected(resp))
        }
        1 => {
            let resp: RefusedResponse = serde_json::from_str(json)?;
            Ok(ServerMessage::Refused(resp))
        }
        2 => {
            let msg: JoinMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Join(msg))
        }
        3 => {
            let msg: LeaveMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Leave(msg))
        }
        4 => {
            #[derive(Deserialize)]
            struct CursorUpdate {
                id: u32,
                col: i32,
                row: i32,
            }
            let msg: CursorUpdate = serde_json::from_str(json)?;
            Ok(ServerMessage::Cursor {
                id: msg.id,
                col: msg.col,
                row: msg.row,
            })
        }
        5 => {
            #[derive(Deserialize)]
            struct SelectionUpdate {
                id: u32,
                selecting: bool,
                col: i32,
                row: i32,
            }
            let msg: SelectionUpdate = serde_json::from_str(json)?;
            Ok(ServerMessage::Selection {
                id: msg.id,
                selecting: msg.selecting,
                col: msg.col,
                row: msg.row,
            })
        }
        6 => {
            let msg: ResizeColumnsMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::ResizeColumns { columns: msg.columns })
        }
        7 => {
            let msg: ResizeRowsMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::ResizeRows { rows: msg.rows })
        }
        8 => {
            let msg: DrawMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Draw(msg))
        }
        9 => {
            let msg: DrawPreviewMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::DrawPreview(msg))
        }
        10 => {
            let msg: ChatBroadcastMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Chat(msg))
        }
        11 => {
            let msg: StatusMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Status(msg))
        }
        15 => {
            let msg: PasteMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::Paste(msg))
        }
        17 => Ok(ServerMessage::Ping),
        18 => {
            let msg: SetUse9pxMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::SetUse9px { value: msg.value })
        }
        19 => {
            let msg: SetIceColorsMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::SetIceColors { value: msg.value })
        }
        20 => {
            let msg: SetFontMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::SetFont { font: msg.font })
        }
        21 => {
            let msg: SetBackgroundMessage = serde_json::from_str(json)?;
            Ok(ServerMessage::SetBackground { value: msg.value })
        }
        other => Ok(ServerMessage::Unknown(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_request_serialization() {
        let req = ConnectRequest::new("TestUser".to_string(), "secret".to_string());
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"action\":16"));
        assert!(json.contains("\"nick\":\"TestUser\""));
        assert!(json.contains("\"pass\":\"secret\""));
    }

    #[test]
    fn test_draw_message_serialization() {
        let msg = DrawMessage::new(10, 20, Block { code: 65, fg: 7, bg: 0 });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"action\":8"));
        assert!(json.contains("\"col\":10"));
        assert!(json.contains("\"row\":20"));
    }

    #[test]
    fn test_draw_message_with_layer() {
        let msg = DrawMessage::with_layer(10, 20, Block { code: 65, fg: 7, bg: 0 }, 2);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"layer\":2"));
    }

    #[test]
    fn test_parse_connected_response() {
        let json = r#"{"action":0,"id":42,"doc":"","users":[],"chat_history":[],"status":{"text":""},"columns":80,"rows":25}"#;
        let msg = parse_server_message(json).unwrap();
        match msg {
            ServerMessage::Connected(resp) => {
                assert_eq!(resp.id, 42);
                assert_eq!(resp.columns, 80);
                assert_eq!(resp.rows, 25);
            }
            _ => panic!("Expected Connected message"),
        }
    }

    #[test]
    fn test_protocol_version_compat() {
        // Moebius-compatible request without version
        let req = ConnectRequest::moebius_compatible("User".to_string(), "".to_string());
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("protocol_version"));

        // Extended request with version
        let req = ConnectRequest::new("User".to_string(), "".to_string());
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("protocol_version"));
    }
}
