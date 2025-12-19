//! Moebius-compatible WebSocket protocol types.
//!
//! This module defines all message types used in the collaboration protocol.
//! The protocol is designed to be fully compatible with Moebius while supporting
//! extensions through protocol versioning.

use serde::{Deserialize, Serialize};

use super::compression::MoebiusCompressedData;

/// Action codes matching Moebius protocol.
/// These are the message types exchanged between client and server.
/// IMPORTANT: These values MUST match the Moebius protocol exactly!
/// Reference: moebius/app/server.js line 3
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
    /// Resize selection (Moebius internal)
    ResizeSelection = 6,
    /// Operation (Moebius internal)
    Operation = 7,
    /// Hide cursor
    HideCursor = 8,
    /// Draw a single character cell
    Draw = 9,
    /// Chat message
    Chat = 10,
    /// Server status update
    Status = 11,
    /// SAUCE metadata update
    Sauce = 12,
    /// Set ice colors mode
    IceColors = 13,
    /// Set use9px (9-pixel font mode / letter spacing)
    Use9pxFont = 14,
    /// Change font
    ChangeFont = 15,
    /// Set canvas size (columns AND rows together)
    SetCanvasSize = 16,
    /// Paste as selection
    PasteAsSelection = 17,
    /// Rotate
    Rotate = 18,
    /// Flip X
    FlipX = 19,
    /// Flip Y
    FlipY = 20,
    /// Set background color for canvas
    SetBackground = 21,
}

impl TryFrom<u8> for ActionCode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ActionCode::Connected),
            1 => Ok(ActionCode::Refused),
            2 => Ok(ActionCode::Join),
            3 => Ok(ActionCode::Leave),
            4 => Ok(ActionCode::Cursor),
            5 => Ok(ActionCode::Selection),
            6 => Ok(ActionCode::ResizeSelection),
            7 => Ok(ActionCode::Operation),
            8 => Ok(ActionCode::HideCursor),
            9 => Ok(ActionCode::Draw),
            10 => Ok(ActionCode::Chat),
            11 => Ok(ActionCode::Status),
            12 => Ok(ActionCode::Sauce),
            13 => Ok(ActionCode::IceColors),
            14 => Ok(ActionCode::Use9pxFont),
            15 => Ok(ActionCode::ChangeFont),
            16 => Ok(ActionCode::SetCanvasSize),
            17 => Ok(ActionCode::PasteAsSelection),
            18 => Ok(ActionCode::Rotate),
            19 => Ok(ActionCode::FlipX),
            20 => Ok(ActionCode::FlipY),
            21 => Ok(ActionCode::SetBackground),
            _ => Err(value),
        }
    }
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Block {
    /// Unicode code point for the character
    pub code: u32,
    /// Foreground color index (0-15 for standard, extended for truecolor)
    pub fg: u8,
    /// Background color index (0-7 for standard, extended for ice colors)
    pub bg: u8,
}

/// A rectangular block buffer used by Moebius for floating selections.
///
/// Wire format matches Moebius `blocks`: `{ columns, rows, data: [ {code,fg,bg}, ... ] }`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Blocks {
    pub columns: u32,
    pub rows: u32,
    #[serde(default)]
    pub data: Vec<Block>,
}

/// RGB color value for truecolor support.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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
    #[serde(default)]
    pub nick: String,
    /// Group tag (Moebius)
    #[serde(default)]
    pub group: String,
    /// Status value (Moebius statuses: ACTIVE=0, IDLE=1, AWAY=2, WEB=3)
    #[serde(default)]
    pub status: u8,

    /// Internal cursor column (not part of Moebius wire format)
    #[serde(default, skip_serializing)]
    pub col: i32,
    /// Internal cursor row (not part of Moebius wire format)
    #[serde(default, skip_serializing)]
    pub row: i32,
    /// Internal selection mode flag (not part of Moebius wire format)
    #[serde(default, skip_serializing)]
    pub selecting: bool,
    /// Internal selection column (not part of Moebius wire format)
    #[serde(default, skip_serializing)]
    pub selection_col: i32,
    /// Internal selection row (not part of Moebius wire format)
    #[serde(default, skip_serializing)]
    pub selection_row: i32,
}

/// Chat message in session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// User id (Moebius chat history/broadcast)
    #[serde(default)]
    pub id: u32,
    /// User nickname who sent the message
    #[serde(default)]
    pub nick: String,
    /// Message text content
    #[serde(default)]
    pub text: String,
    /// Group/channel name (for Moebius compatibility)
    #[serde(default)]
    pub group: String,
    /// Timestamp (optional, server-assigned)
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub time: u64,
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

/// Server status for display.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStatus {
    /// User id for which this status applies
    pub id: u32,
    /// Status code (Moebius statuses: ACTIVE=0, IDLE=1, AWAY=2, WEB=3)
    pub status: u8,
}

/// SAUCE metadata update.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SauceData {
    /// User id who made the change
    #[serde(default)]
    pub id: u32,
    /// Document title (max 35 chars)
    #[serde(default)]
    pub title: String,
    /// Author name (max 20 chars)
    #[serde(default)]
    pub author: String,
    /// Group name (max 20 chars)
    #[serde(default)]
    pub group: String,
    /// Comments
    #[serde(default)]
    pub comments: String,
}

/// Data received when successfully connected to a collaboration server.
/// Contains the initial document state and session information.
#[derive(Debug, Clone)]
pub struct ConnectedDocument {
    /// Assigned user ID for this session
    pub user_id: u32,
    /// Decoded document blocks (column-major layout)
    pub document: Vec<Vec<Block>>,
    /// Document width in columns
    pub columns: u32,
    /// Document height in rows
    pub rows: u32,
    /// List of users already in the session
    pub users: Vec<User>,
    /// Chat history snapshot (if provided by server on connect)
    pub chat_history: Vec<ChatMessage>,
    /// Whether 9px font mode is enabled
    pub use_9px: bool,
    /// Whether ice colors are enabled
    pub ice_colors: bool,
    /// Font name
    pub font: String,
    /// SAUCE title
    pub title: String,
    /// SAUCE author
    pub author: String,
    /// SAUCE group
    pub group: String,
    /// SAUCE comments
    pub comments: String,
}

/// Generic Moebius wire message: `{ "type": <u8>, "data": { ... } }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoebiusMessage<T> {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: T,
}

/// Moebius document payload (result of `libtextmode.compress(doc)`).
///
/// The payload contains metadata and either `compressed_data` (most common) or
/// an uncompressed `data` array of blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoebiusDoc {
    pub columns: u32,
    pub rows: u32,

    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub date: String,

    /// Palette is an array in Moebius; we treat it as opaque JSON.
    #[serde(default)]
    pub palette: serde_json::Value,

    #[serde(default)]
    pub font_name: String,
    #[serde(default)]
    pub ice_colors: bool,
    #[serde(default)]
    pub use_9px_font: bool,
    #[serde(default)]
    pub comments: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c64_background: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_data: Option<MoebiusCompressedData>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Block>>,
}

impl MoebiusDoc {
    /// Convert wire-format MoebiusDoc into a ConnectedDocument.
    ///
    /// Decompresses the document data and maps field names to the internal format.
    pub fn into_connected_document(self, user_id: u32, users: Vec<User>) -> Result<ConnectedDocument, super::compression::CompressionError> {
        use super::compression::{flat_to_columns, uncompress_moebius_data};

        let flat_blocks = if let Some(compressed) = &self.compressed_data {
            uncompress_moebius_data(self.columns, self.rows, compressed)?
        } else {
            self.data.clone().unwrap_or_default()
        };

        let document = flat_to_columns(&flat_blocks, self.columns, self.rows);

        Ok(ConnectedDocument {
            user_id,
            document,
            columns: self.columns,
            rows: self.rows,
            users,
            chat_history: Vec::new(),
            use_9px: self.use_9px_font,
            ice_colors: self.ice_colors,
            font: self.font_name,
            title: self.title,
            author: self.author,
            group: self.group,
            comments: self.comments,
        })
    }
}

// ============================================================================
// Client -> Server Messages
// ============================================================================

/// Connect request sent by client to join a session.
/// In Moebius, client sends CONNECTED (0) to initiate connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ConnectData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectData {
    #[serde(default)]
    pub nick: String,
    #[serde(default)]
    pub group: String,
    pub pass: String,
}

impl ConnectRequest {
    pub fn moebius_compatible(nick: String, group: String, password: String) -> Self {
        Self {
            msg_type: ActionCode::Connected as u8,
            data: ConnectData { nick, group, pass: password },
        }
    }
}

/// Cursor position update from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: CursorData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorData {
    pub id: u32,
    pub x: i32,
    pub y: i32,
}

impl CursorMessage {
    pub fn new(id: u32, x: i32, y: i32) -> Self {
        Self {
            msg_type: ActionCode::Cursor as u8,
            data: CursorData { id, x, y },
        }
    }
}

/// Selection update from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: SelectionData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionData {
    pub id: u32,
    pub x: i32,
    pub y: i32,
}

impl SelectionMessage {
    pub fn new(id: u32, x: i32, y: i32) -> Self {
        Self {
            msg_type: ActionCode::Selection as u8,
            data: SelectionData { id, x, y },
        }
    }
}

/// Draw a single character cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: DrawData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawData {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub block: Block,
    /// Optional extension field (not used by Moebius).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
}

impl DrawMessage {
    pub fn new(id: u32, x: i32, y: i32, block: Block) -> Self {
        Self {
            msg_type: ActionCode::Draw as u8,
            data: DrawData { id, x, y, block, layer: None },
        }
    }

    pub fn with_layer(id: u32, x: i32, y: i32, block: Block, layer: usize) -> Self {
        Self {
            msg_type: ActionCode::Draw as u8,
            data: DrawData {
                id,
                x,
                y,
                block,
                layer: Some(layer),
            },
        }
    }
}

/// Preview draw - Moebius doesn't have separate preview, use regular Draw
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawPreviewMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: DrawData,
}

impl DrawPreviewMessage {
    pub fn new(id: u32, x: i32, y: i32, block: Block) -> Self {
        Self {
            msg_type: ActionCode::Draw as u8,
            data: DrawData { id, x, y, block, layer: None },
        }
    }
}

/// Chat message from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ChatSendData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSendData {
    pub id: u32,
    #[serde(default)]
    pub nick: String,
    #[serde(default)]
    pub group: String,
    pub text: String,
}

impl ChatSendMessage {
    pub fn new(id: u32, nick: String, group: String, text: String) -> Self {
        Self {
            msg_type: ActionCode::Chat as u8,
            data: ChatSendData { id, nick, group, text },
        }
    }
}

/// Resize canvas - Moebius uses SET_CANVAS_SIZE (16) with both columns and rows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeColumnsMessage {
    pub action: u8,
    pub columns: u32,
}

impl ResizeColumnsMessage {
    pub fn new(columns: u32) -> Self {
        Self {
            action: ActionCode::SetCanvasSize as u8, // Not supported separately in Moebius
            columns,
        }
    }
}

/// Resize canvas rows - Moebius uses SET_CANVAS_SIZE (16) with both columns and rows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeRowsMessage {
    pub action: u8,
    pub rows: u32,
}

impl ResizeRowsMessage {
    pub fn new(rows: u32) -> Self {
        Self {
            action: ActionCode::SetCanvasSize as u8, // Not supported separately in Moebius
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

/// Ping message for keepalive - Not in Moebius protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    pub action: u8,
}

impl Default for PingMessage {
    fn default() -> Self {
        Self {
            action: ActionCode::Status as u8, // Use Status as fallback since Ping doesn't exist
        }
    }
}

/// Set 9-pixel font mode - Moebius USE_9PX_FONT = 14
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetUse9pxMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ToggleValueData,
}

impl SetUse9pxMessage {
    pub fn new(id: u32, value: bool) -> Self {
        Self {
            msg_type: ActionCode::Use9pxFont as u8,
            data: ToggleValueData { id, value },
        }
    }
}

/// Set ice colors mode - Moebius ICE_COLORS = 13
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetIceColorsMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ToggleValueData,
}

impl SetIceColorsMessage {
    pub fn new(id: u32, value: bool) -> Self {
        Self {
            msg_type: ActionCode::IceColors as u8,
            data: ToggleValueData { id, value },
        }
    }
}

/// Set font - Moebius CHANGE_FONT = 15
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFontMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: SetFontData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleValueData {
    pub id: u32,
    pub value: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFontData {
    pub id: u32,
    pub font_name: String,
}

impl SetFontMessage {
    pub fn new(id: u32, font_name: String) -> Self {
        Self {
            msg_type: ActionCode::ChangeFont as u8,
            data: SetFontData { id, font_name },
        }
    }
}

/// Set canvas size - Moebius SET_CANVAS_SIZE = 16
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCanvasSizeMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: SetCanvasSizeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCanvasSizeData {
    pub id: u32,
    pub columns: u32,
    pub rows: u32,
}

impl SetCanvasSizeMessage {
    pub fn new(id: u32, columns: u32, rows: u32) -> Self {
        Self {
            msg_type: ActionCode::SetCanvasSize as u8,
            data: SetCanvasSizeData { id, columns, rows },
        }
    }
}

/// Set background color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBackgroundMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: SetBackgroundData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBackgroundData {
    pub id: u32,
    pub value: u32,
}

impl SetBackgroundMessage {
    pub fn new(id: u32, value: u32) -> Self {
        Self {
            msg_type: ActionCode::SetBackground as u8,
            data: SetBackgroundData { id, value },
        }
    }
}

// ============================================================================
// Server -> Client Messages
// ============================================================================

/// Connection accepted response with document state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedResponse {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ConnectedData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedData {
    pub id: u32,
    pub doc: MoebiusDoc,

    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub chat_history: Vec<ChatMessage>,
    #[serde(default)]
    pub status: u8,
}

/// Connection refused response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusedResponse {
    #[serde(rename = "type")]
    pub msg_type: u8,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// User joined notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: JoinData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinData {
    pub id: u32,
    #[serde(default)]
    pub nick: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub status: u8,
}

/// User left notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: LeaveData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveData {
    pub id: u32,
}

/// Chat message broadcast from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBroadcastMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ChatMessage,
}

/// Status update from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: ServerStatus,
}

// ============================================================================
// Generic Message Parsing
// ============================================================================

/// Incoming message that can be any action type.
/// Use this for initial parsing to determine the action code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub data: serde_json::Value,
}

impl IncomingMessage {
    /// Get the action code for this message.
    /// Maps Moebius protocol action codes to our ActionCode enum.
    pub fn action_code(&self) -> Option<ActionCode> {
        ActionCode::try_from(self.msg_type).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collaboration::ClientCommand;
    use crate::collaboration::user_status;
    use serde_json::json;

    // ========================================================================
    // ActionCode Tests
    // ========================================================================

    #[test]
    fn action_code_try_from_valid() {
        assert_eq!(ActionCode::try_from(0), Ok(ActionCode::Connected));
        assert_eq!(ActionCode::try_from(1), Ok(ActionCode::Refused));
        assert_eq!(ActionCode::try_from(2), Ok(ActionCode::Join));
        assert_eq!(ActionCode::try_from(3), Ok(ActionCode::Leave));
        assert_eq!(ActionCode::try_from(4), Ok(ActionCode::Cursor));
        assert_eq!(ActionCode::try_from(5), Ok(ActionCode::Selection));
        assert_eq!(ActionCode::try_from(9), Ok(ActionCode::Draw));
        assert_eq!(ActionCode::try_from(10), Ok(ActionCode::Chat));
        assert_eq!(ActionCode::try_from(11), Ok(ActionCode::Status));
        assert_eq!(ActionCode::try_from(12), Ok(ActionCode::Sauce));
        assert_eq!(ActionCode::try_from(16), Ok(ActionCode::SetCanvasSize));
        assert_eq!(ActionCode::try_from(21), Ok(ActionCode::SetBackground));
    }

    #[test]
    fn action_code_try_from_invalid() {
        assert!(ActionCode::try_from(22).is_err());
        assert!(ActionCode::try_from(100).is_err());
        assert!(ActionCode::try_from(255).is_err());
    }

    // ========================================================================
    // User Serialization Tests
    // ========================================================================

    #[test]
    fn user_serialization() {
        let user = User {
            id: 42,
            nick: "TestUser".to_string(),
            group: "TestGroup".to_string(),
            status: user_status::ACTIVE,
            col: 10,
            row: 20,
            selecting: true,
            selection_col: 5,
            selection_row: 15,
        };

        let json = serde_json::to_string(&user).unwrap();
        let parsed: User = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 42);
        assert_eq!(parsed.nick, "TestUser");
        assert_eq!(parsed.group, "TestGroup");
        assert_eq!(parsed.status, user_status::ACTIVE);
    }

    #[test]
    fn user_deserialization_with_defaults() {
        // Moebius sends minimal user data
        let json = json!({
            "id": 5,
            "nick": "Guest",
            "status": 1
        });

        let user: User = serde_json::from_value(json).unwrap();

        assert_eq!(user.id, 5);
        assert_eq!(user.nick, "Guest");
        assert_eq!(user.group, ""); // default
        assert_eq!(user.status, user_status::IDLE);
        assert_eq!(user.col, 0); // default
        assert_eq!(user.row, 0); // default
    }

    // ========================================================================
    // ChatMessage Serialization Tests
    // ========================================================================

    #[test]
    fn chat_message_serialization() {
        let msg = ChatMessage {
            id: 1,
            nick: "Alice".to_string(),
            text: "Hello, world!".to_string(),
            group: "Blocktronics".to_string(),
            time: 1234567890,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.nick, "Alice");
        assert_eq!(parsed.text, "Hello, world!");
        assert_eq!(parsed.group, "Blocktronics");
        assert_eq!(parsed.time, 1234567890);
    }

    #[test]
    fn chat_message_deserialization_with_defaults() {
        let json = json!({
            "id": 2,
            "nick": "Bob",
            "text": "Hi!"
        });

        let msg: ChatMessage = serde_json::from_value(json).unwrap();

        assert_eq!(msg.id, 2);
        assert_eq!(msg.nick, "Bob");
        assert_eq!(msg.text, "Hi!");
        assert_eq!(msg.group, ""); // default
        assert_eq!(msg.time, 0); // default
    }

    // ========================================================================
    // Block Serialization Tests
    // ========================================================================

    #[test]
    fn block_serialization() {
        let block = Block {
            code: 65, // 'A'
            fg: 7,
            bg: 0,
        };

        let json = serde_json::to_string(&block).unwrap();
        let parsed: Block = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.code, 65);
        assert_eq!(parsed.fg, 7);
        assert_eq!(parsed.bg, 0);
    }

    #[test]
    fn block_with_extended_colors() {
        let block = Block {
            code: 219, // Full block
            fg: 15,    // Bright white
            bg: 4,     // Red
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"code\":219"));
        assert!(json.contains("\"fg\":15"));
        assert!(json.contains("\"bg\":4"));
    }

    // ========================================================================
    // Blocks Collection Serialization Tests
    // ========================================================================

    #[test]
    fn blocks_serialization() {
        let blocks = Blocks {
            columns: 2,
            rows: 2,
            data: vec![
                Block { code: 65, fg: 7, bg: 0 },
                Block { code: 66, fg: 2, bg: 1 },
                Block { code: 67, fg: 3, bg: 2 },
                Block { code: 68, fg: 4, bg: 3 },
            ],
        };

        let json = serde_json::to_string(&blocks).unwrap();
        let parsed: Blocks = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.columns, 2);
        assert_eq!(parsed.rows, 2);
        assert_eq!(parsed.data.len(), 4);
        assert_eq!(parsed.data[0].code, 65);
        assert_eq!(parsed.data[3].code, 68);
    }

    // ========================================================================
    // IncomingMessage Tests
    // ========================================================================

    #[test]
    fn incoming_message_action_code() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": 10,
            "data": { "id": 1, "nick": "Test", "text": "Hello" }
        }))
        .unwrap();

        assert_eq!(msg.action_code(), Some(ActionCode::Chat));
    }

    #[test]
    fn incoming_message_unknown_action() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": 99,
            "data": {}
        }))
        .unwrap();

        assert_eq!(msg.action_code(), None);
    }

    // ========================================================================
    // ClientCommand Serialization Tests
    // ========================================================================

    #[test]
    fn client_command_cursor() {
        let cmd = ClientCommand::Cursor { col: 10, row: 20 };

        if let ClientCommand::Cursor { col, row } = cmd {
            assert_eq!(col, 10);
            assert_eq!(row, 20);
        } else {
            panic!("Expected Cursor command");
        }
    }

    #[test]
    fn client_command_draw() {
        let block = Block { code: 65, fg: 7, bg: 0 };
        let cmd = ClientCommand::Draw {
            col: 5,
            row: 10,
            block: block.clone(),
        };

        if let ClientCommand::Draw { col, row, block: b } = cmd {
            assert_eq!(col, 5);
            assert_eq!(row, 10);
            assert_eq!(b.code, 65);
        } else {
            panic!("Expected Draw command");
        }
    }

    #[test]
    fn client_command_chat() {
        let cmd = ClientCommand::Chat {
            text: "Hello, world!".to_string(),
        };

        if let ClientCommand::Chat { text } = cmd {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected Chat command");
        }
    }

    #[test]
    fn client_command_set_canvas_size() {
        let cmd = ClientCommand::SetCanvasSize { columns: 160, rows: 50 };

        if let ClientCommand::SetCanvasSize { columns, rows } = cmd {
            assert_eq!(columns, 160);
            assert_eq!(rows, 50);
        } else {
            panic!("Expected SetCanvasSize command");
        }
    }

    // ========================================================================
    // User Status Constants Tests
    // ========================================================================

    #[test]
    fn user_status_constants() {
        assert_eq!(user_status::ACTIVE, 0);
        assert_eq!(user_status::IDLE, 1);
        assert_eq!(user_status::AWAY, 2);
        assert_eq!(user_status::WEB, 3);
    }

    // ========================================================================
    // SauceData Serialization Tests
    // ========================================================================

    #[test]
    fn sauce_data_serialization() {
        let sauce = SauceData {
            id: 1,
            title: "My Art".to_string(),
            author: "Artist".to_string(),
            group: "Crew".to_string(),
            comments: "Line 1\nLine 2".to_string(),
        };

        let json = serde_json::to_string(&sauce).unwrap();
        let parsed: SauceData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.title, "My Art");
        assert_eq!(parsed.author, "Artist");
        assert_eq!(parsed.group, "Crew");
        assert_eq!(parsed.comments, "Line 1\nLine 2");
    }

    // ========================================================================
    // ConnectedDocument Tests
    // ========================================================================

    #[test]
    fn connected_document_fields() {
        let doc = ConnectedDocument {
            user_id: 42,
            document: vec![vec![Block { code: 65, fg: 7, bg: 0 }]],
            columns: 80,
            rows: 25,
            users: vec![User {
                id: 1,
                nick: "Other".to_string(),
                group: "G".to_string(),
                status: 0,
                col: 0,
                row: 0,
                selecting: false,
                selection_col: 0,
                selection_row: 0,
            }],
            chat_history: vec![ChatMessage {
                id: 1,
                nick: "Other".to_string(),
                text: "Hi".to_string(),
                group: "G".to_string(),
                time: 123,
            }],
            use_9px: false,
            ice_colors: true,
            font: "IBM VGA".to_string(),
            title: "Test".to_string(),
            author: "Author".to_string(),
            group: "Group".to_string(),
            comments: "Comments".to_string(),
        };

        assert_eq!(doc.user_id, 42);
        assert_eq!(doc.columns, 80);
        assert_eq!(doc.rows, 25);
        assert_eq!(doc.users.len(), 1);
        assert_eq!(doc.chat_history.len(), 1);
        assert!(doc.ice_colors);
        assert!(!doc.use_9px);
    }

    /// Test that buffer and layer sizes remain consistent after applying a remote document.
    /// This is a regression test for a bug where the buffer would show only 25 rows
    /// when the document was 50 rows, because layer 0 size was not properly synchronized.
    #[test]
    fn buffer_layer_size_consistency_after_remote_document() {
        use icy_engine::{TextBuffer, TextPane, Position, AttributedChar};
        
        // Create a "remote" document with 80x50 (50 rows - more than default 25)
        let mut document = Vec::new();
        for _col in 0..80 {
            let mut column = Vec::new();
            for row in 0..50 {
                column.push(Block {
                    code: if row == 49 { 'X' as u32 } else { ' ' as u32 },
                    fg: 7,
                    bg: 0,
                });
            }
            document.push(column);
        }
        
        let remote_doc = ConnectedDocument {
            user_id: 1,
            document,
            columns: 80,
            rows: 50,  // 50 rows, not 25!
            users: vec![],
            chat_history: vec![],
            use_9px: false,
            ice_colors: false,
            font: String::new(),
            title: String::new(),
            author: String::new(),
            group: String::new(),
            comments: String::new(),
        };
        
        // Simulate what icy_draw does: create a buffer with default 80x25
        let mut buffer = TextBuffer::new((80, 25));
        buffer.terminal_state.is_terminal_buffer = false;
        
        // Apply the remote document (mimics apply_remote_document)
        let cols_i32 = remote_doc.columns as i32;
        let rows_i32 = remote_doc.rows as i32;
        
        // Set document size
        buffer.set_size((cols_i32, rows_i32));
        buffer.layers[0].set_size((cols_i32, rows_i32));
        
        // Resize and preallocate layer 0 for fast bulk writes
        let layer = &mut buffer.layers[0];
        layer.preallocate_lines(cols_i32, rows_i32);
        
        for col in 0..(remote_doc.columns as usize) {
            for row in 0..(remote_doc.rows as usize) {
                let block = remote_doc.document.get(col).and_then(|c| c.get(row)).cloned().unwrap_or_default();
                
                let mut ch = AttributedChar::default();
                ch.ch = char::from_u32(block.code).unwrap_or(' ');
                ch.attribute.set_foreground(block.fg as u32);
                ch.attribute.set_background(block.bg as u32);
                
                layer.set_char_unchecked(Position::new(col as i32, row as i32), ch);
            }
        }
        
        // Now verify consistency
        assert_eq!(buffer.width(), 80, "Buffer width should be 80");
        assert_eq!(buffer.height(), 50, "Buffer height should be 50");
        
        assert_eq!(buffer.layers[0].size().width, 80, "Layer 0 width should be 80");
        assert_eq!(buffer.layers[0].size().height, 50, "Layer 0 height should be 50");
        
        // Most importantly: the actual line count should match!
        assert_eq!(buffer.layers[0].lines.len(), 50, "Layer 0 should have 50 lines allocated");
        
        // Verify we can access the last row
        let last_char = buffer.char_at(Position::new(0, 49));
        assert_eq!(last_char.ch, 'X', "Should be able to read character at row 49");
    }
    
    /// Test that a Moebius CONNECTED response with 50 rows is correctly parsed.
    /// This tests the full deserialization pipeline.
    #[test]
    fn connected_response_50_rows_parsing() {
        use crate::collaboration::compression::MoebiusCompressedData;
        
        // Simulate compressed data for 80x50 document (4000 blocks)
        // All blocks are the same: code=32 (space), fg=7, bg=0
        let compressed = MoebiusCompressedData {
            code: vec![[32, 3999]], // one run of 4000 spaces
            fg: vec![[7, 3999]],    // one run of 4000 foreground=7
            bg: vec![[0, 3999]],    // one run of 4000 background=0
        };
        
        let moebius_doc = MoebiusDoc {
            columns: 80,
            rows: 50,  // 50 rows, not 25!
            title: "Test".to_string(),
            author: "Author".to_string(),
            group: "Group".to_string(),
            date: "".to_string(),
            palette: serde_json::Value::Null,
            font_name: "IBM VGA".to_string(),
            ice_colors: false,
            use_9px_font: false,
            comments: "".to_string(),
            c64_background: None,
            compressed_data: Some(compressed),
            data: None,
        };
        
        let result = moebius_doc.into_connected_document(1, vec![]).unwrap();
        
        assert_eq!(result.columns, 80, "Document columns should be 80");
        assert_eq!(result.rows, 50, "Document rows should be 50");
        assert_eq!(result.document.len(), 80, "Document should have 80 columns");
        assert_eq!(result.document[0].len(), 50, "Each column should have 50 rows");
    }
}
