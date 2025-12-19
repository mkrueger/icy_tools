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
