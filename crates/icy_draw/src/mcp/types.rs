//! MCP request/response types for icy_draw

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
// General types
// ═══════════════════════════════════════════════════════════════════════════════

/// Screen capture format
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScreenCaptureFormat {
    /// ANSI escape codes
    Ansi,
    /// Plain text (no colors)
    Text,
}

/// Editor status response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditorStatus {
    /// Current editor mode
    pub editor: String,
    /// Open file path (if any)
    pub file: Option<String>,
    /// Whether document has unsaved changes
    pub dirty: bool,
    /// ANSI-specific status (if in ansi mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ansi: Option<AnsiStatus>,
    /// Animation-specific status (if in animation mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationStatus>,
    /// BitFont-specific status (if in bitfont mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitfont: Option<BitFontStatus>,
}

/// Animation editor status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationStatus {
    /// Length of Lua script in bytes
    pub text_length: usize,
    /// Number of rendered frames
    pub frame_count: usize,
    /// Script errors (empty if none)
    pub errors: Vec<String>,
    /// Whether animation is currently playing
    pub is_playing: bool,
    /// Current frame number
    pub current_frame: usize,
}

/// BitFont editor status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BitFontStatus {
    /// Width of glyphs in pixels
    pub glyph_width: i32,
    /// Height of glyphs in pixels
    pub glyph_height: i32,
    /// Number of glyphs in font
    pub glyph_count: usize,
    /// First character code
    pub first_char: u32,
    /// Last character code
    pub last_char: u32,
    /// Currently selected character code
    pub selected_char: u32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetHelpRequest {
    /// Optional editor type: "animation", "bitfont", or omit for general help
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewDocumentRequest {
    /// Document type: "ansi", "animation", "bitfont", "charfont"
    #[serde(rename = "type")]
    pub doc_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoadDocumentRequest {
    /// Path to file to open
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationGetTextRequest {
    /// Byte offset to start reading from (optional, default 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// Number of bytes to read (optional, default all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationReplaceTextRequest {
    /// Byte offset where replacement starts
    pub offset: usize,
    /// Number of bytes to replace
    pub length: usize,
    /// New text to insert
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationGetScreenRequest {
    /// Frame number (1-based)
    pub frame: usize,
    /// Output format (default: ansi)
    #[serde(default = "default_screen_format")]
    pub format: ScreenCaptureFormat,
}

fn default_screen_format() -> ScreenCaptureFormat {
    ScreenCaptureFormat::Ansi
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BitFontGetCharRequest {
    /// Character code (0-255 typically)
    pub code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BitFontSetCharRequest {
    /// Character code
    pub code: u32,
    /// Glyph data
    pub data: GlyphData,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiRunScriptRequest {
    /// The Lua script code to execute
    pub script: String,
    /// Optional description for the undo stack (default: "MCP Script")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo_description: Option<String>,
}

/// Response from running a Lua script
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiRunScriptResponse {
    /// Whether the script executed successfully
    pub success: bool,
    /// Output/log messages from the script
    pub output: Vec<String>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI MCP request types
// ═══════════════════════════════════════════════════════════════════════════════

/// Output format for ANSI editor screen capture
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnsiScreenFormat {
    /// ANSI escape codes
    Ansi,
    /// Plain text (no colors, no images)
    Ascii,
}

fn default_ansi_screen_format() -> AnsiScreenFormat {
    AnsiScreenFormat::Ansi
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiGetScreenRequest {
    /// Output format (default: ansi)
    #[serde(default = "default_ansi_screen_format")]
    pub format: AnsiScreenFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetCaretRequest {
    /// X position (layer-relative)
    pub x: i32,
    /// Y position (layer-relative)
    pub y: i32,
    /// Caret text attribute
    pub attribute: TextAttributeInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiAddLayerRequest {
    /// Insert new layer after this layer index
    pub after_layer: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiDeleteLayerRequest {
    /// Layer index (0-based)
    pub layer: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetLayerPropsRequest {
    /// Layer index (0-based)
    pub layer: usize,
    /// Layer title/name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Visibility (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_visible: Option<bool>,
    /// Edit lock (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_locked: Option<bool>,
    /// Position lock (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_position_locked: Option<bool>,
    /// Offset X (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_x: Option<i32>,
    /// Offset Y (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset_y: Option<i32>,
    /// Layer transparency (0-255) (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiMergeDownLayerRequest {
    /// Layer index (0-based)
    pub layer: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LayerMoveDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiMoveLayerRequest {
    /// Layer index (0-based)
    pub layer: usize,
    /// Move direction
    pub direction: LayerMoveDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiResizeRequest {
    /// New buffer width
    pub width: i32,
    /// New buffer height
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiGetRegionRequest {
    /// Layer index (0-based)
    pub layer: usize,
    /// X start (layer coordinates)
    pub x: i32,
    /// Y start (layer coordinates)
    pub y: i32,
    /// Region width
    pub width: i32,
    /// Region height
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegionData {
    /// Layer index (0-based)
    pub layer: usize,
    /// X start (layer coordinates)
    pub x: i32,
    /// Y start (layer coordinates)
    pub y: i32,
    /// Region width
    pub width: i32,
    /// Region height
    pub height: i32,
    /// Character data (row-major order, width * height elements)
    pub chars: Vec<CharInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetRegionRequest {
    /// Layer index (0-based)
    pub layer: usize,
    /// X start (layer coordinates)
    pub x: i32,
    /// Y start (layer coordinates)
    pub y: i32,
    /// Region width
    pub width: i32,
    /// Region height
    pub height: i32,
    /// Character data (row-major order, width * height elements)
    pub chars: Vec<CharInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetSelectionRequest {
    /// Selection rectangle X
    pub x: i32,
    /// Selection rectangle Y
    pub y: i32,
    /// Selection rectangle width
    pub width: i32,
    /// Selection rectangle height
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSelectionActionRequest {
    /// Action name (e.g. "flip_x", "flip_y", "justify_left", "justify_center", "justify_right", "crop")
    pub action: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Response types
// ═══════════════════════════════════════════════════════════════════════════════

/// Glyph bitmap data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GlyphData {
    /// Character code
    pub code: u32,
    /// Character as string (if printable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    /// Width in pixels
    pub width: i32,
    /// Height in pixels
    pub height: i32,
    /// Base64-encoded bitmap (1 bit per pixel, row-major)
    pub bitmap: String,
}

/// List of character codes response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CharListResponse {
    /// Array of character codes
    pub chars: Vec<u32>,
    /// Total count
    pub count: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI Editor types
// ═══════════════════════════════════════════════════════════════════════════════

/// ANSI editor status
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiStatus {
    /// Buffer information
    pub buffer: BufferInfo,
    /// Caret/cursor information
    pub caret: CaretInfo,
    /// Layer information
    pub layers: Vec<LayerInfo>,
    /// Current layer index
    pub current_layer: usize,
    /// Selection information (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionInfo>,
    /// Document format mode
    pub format_mode: String,
    /// Outline style index
    pub outline_style: usize,
    /// Mirror mode enabled
    pub mirror_mode: bool,
}

/// Buffer dimensions and metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BufferInfo {
    /// Width in character cells
    pub width: i32,
    /// Height in character cells
    pub height: i32,
    /// Number of layers
    pub layer_count: usize,
    /// Number of fonts
    pub font_count: usize,
    /// Font mode
    pub font_mode: String,
    /// Ice mode (blink behavior)
    pub ice_mode: String,

    pub palette: String,
}

/// Caret (cursor) position and attributes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaretInfo {
    /// X position (layer-relative)
    pub x: i32,
    /// Y position (layer-relative)
    pub y: i32,
    /// X position (document absolute)
    pub doc_x: i32,
    /// Y position (document absolute)
    pub doc_y: i32,
    /// Current text attribute
    pub attribute: TextAttributeInfo,
    /// Insert mode (true) or overwrite mode (false)
    pub insert_mode: bool,
    /// Current font page
    pub font_page: u8,
}

/// Text attribute details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextAttributeInfo {
    /// Foreground color
    pub foreground: ColorInfo,
    /// Background color
    pub background: ColorInfo,
    /// Bold flag
    pub bold: bool,
    /// Blink flag
    pub blink: bool,
}

/// Color representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "value")]
pub enum ColorInfo {
    /// Palette index (0-15 typically)
    Palette(u8),
    /// Extended palette index (0-255)
    ExtendedPalette(u8),
    /// RGB color
    Rgb { r: u8, g: u8, b: u8 },
    /// Transparent
    Transparent,
}

/// Layer information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerInfo {
    /// Layer index
    pub index: usize,
    /// Layer title/name
    pub title: String,
    /// Whether layer is visible
    pub is_visible: bool,
    /// Whether layer is locked for editing
    pub is_locked: bool,
    /// Whether layer position is locked
    pub is_position_locked: bool,
    /// Layer offset X
    pub offset_x: i32,
    /// Layer offset Y
    pub offset_y: i32,
    /// Layer width
    pub width: i32,
    /// Layer height
    pub height: i32,
    /// Layer transparency (0-255)
    pub transparency: u8,
    /// Layer mode
    pub mode: String,
    /// Layer role
    pub role: String,
}

/// Selection information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectionInfo {
    /// Anchor X position
    pub anchor_x: i32,
    /// Anchor Y position
    pub anchor_y: i32,
    /// Lead X position (current end)
    pub lead_x: i32,
    /// Lead Y position (current end)
    pub lead_y: i32,
    /// Selection shape
    pub shape: String,
    /// Whether selection is locked
    pub locked: bool,
    /// Bounding rectangle
    pub bounds: RectangleInfo,
}

/// Rectangle bounds
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RectangleInfo {
    /// Left edge X
    pub x: i32,
    /// Top edge Y
    pub y: i32,
    /// Width
    pub width: i32,
    /// Height
    pub height: i32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// ANSI Editor Layer Data types
// ═══════════════════════════════════════════════════════════════════════════════

/// Request for getting layer data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiGetLayerRequest {
    /// Layer index (0-based)
    pub layer: usize,
}

/// Full layer data including character data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerData {
    /// Layer index
    pub index: usize,
    /// Layer title/name
    pub title: String,
    /// Whether layer is visible
    pub is_visible: bool,
    /// Whether layer is locked for editing
    pub is_locked: bool,
    /// Whether layer position is locked
    pub is_position_locked: bool,
    /// Layer offset X
    pub offset_x: i32,
    /// Layer offset Y
    pub offset_y: i32,
    /// Layer width
    pub width: i32,
    /// Layer height
    pub height: i32,
    /// Layer transparency (0-255)
    pub transparency: u8,
    /// Layer mode
    pub mode: String,
    /// Layer role
    pub role: String,
    /// Character data (row-major order, width * height elements)
    pub chars: Vec<CharInfo>,
}

/// Character cell information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CharInfo {
    /// Character as Unicode string
    pub ch: String,
    /// Foreground color
    pub fg: ColorInfo,
    /// Background color
    pub bg: ColorInfo,
    /// Font page
    pub font_page: u8,
    /// Bold flag
    pub bold: bool,
    /// Blink flag
    pub blink: bool,
    /// Whether the character is visible
    pub is_visible: bool,
}

/// Request for setting a character
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetCharRequest {
    /// Layer index (0-based)
    pub layer: usize,
    /// X position in layer coordinates
    pub x: i32,
    /// Y position in layer coordinates
    pub y: i32,
    /// Character as Unicode string (single char)
    pub ch: String,
    /// Text attribute for the character
    pub attribute: TextAttributeInfo,
}

/// Request for setting a palette color
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnsiSetColorRequest {
    /// Palette index (0-255)
    pub index: u8,
    /// Red component (0-255)
    pub r: u8,
    /// Green component (0-255)
    pub g: u8,
    /// Blue component (0-255)
    pub b: u8,
}
