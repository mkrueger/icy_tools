//! MCP (Model Context Protocol) server for icy_draw
//!
//! Provides remote control and automation capabilities for icy_draw via HTTP.
//! Enables AI assistants to interact with the editor programmatically.

pub mod handlers;
pub mod server;
pub mod types;

use parking_lot::Mutex;
use std::sync::Arc;

pub use server::*;
use tokio::sync::oneshot;

use crate::mcp::types::{EditorStatus, ScreenCaptureFormat};

pub type SenderType<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

/// Commands sent from MCP server to the UI thread
#[derive(Debug)]
pub enum McpCommand {
    // ═══════════════════════════════════════════════════════════════════════
    // General commands
    // ═══════════════════════════════════════════════════════════════════════
    /// Get help documentation
    GetHelp {
        /// Optional: "animation", "bitfont", or None for general help
        editor_type: Option<String>,
        response: SenderType<String>,
    },

    /// Get current editor status
    GetStatus(SenderType<EditorStatus>),

    /// Create new document
    NewDocument {
        /// "ansi", "animation", "bitfont", "charfont"
        doc_type: String,
        response: SenderType<Result<(), String>>,
    },

    /// Load document from path
    LoadDocument { path: String, response: SenderType<Result<(), String>> },

    /// Save current document
    Save(SenderType<Result<(), String>>),

    /// Undo last action
    Undo(SenderType<Result<(), String>>),

    /// Redo undone action
    Redo(SenderType<Result<(), String>>),

    // ═══════════════════════════════════════════════════════════════════════
    // Animation editor commands
    // ═══════════════════════════════════════════════════════════════════════
    /// Get Lua script text
    AnimationGetText {
        offset: Option<usize>,
        length: Option<usize>,
        response: SenderType<Result<String, String>>,
    },

    /// Replace text in Lua script
    AnimationReplaceText {
        offset: usize,
        length: usize,
        text: String,
        response: SenderType<Result<(), String>>,
    },

    /// Get rendered frame as ANSI/text
    AnimationGetScreen {
        frame: usize,
        format: ScreenCaptureFormat,
        response: SenderType<Result<String, String>>,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // BitFont editor commands
    // ═══════════════════════════════════════════════════════════════════════
    /// List all character codes in font
    BitFontListChars(SenderType<Result<Vec<u32>, String>>),

    /// Get glyph bitmap
    BitFontGetChar {
        code: u32,
        response: SenderType<Result<types::GlyphData, String>>,
    },

    /// Set glyph bitmap
    BitFontSetChar {
        code: u32,
        data: types::GlyphData,
        response: SenderType<Result<(), String>>,
    },

    // ═══════════════════════════════════════════════════════════════════════
    // ANSI editor commands
    // ═══════════════════════════════════════════════════════════════════════
    /// Run a Lua script on the current buffer (like a plugin)
    AnsiRunScript {
        /// The Lua script code to execute
        script: String,
        /// Optional description for undo stack
        undo_description: Option<String>,
        response: SenderType<Result<String, String>>,
    },

    /// Get full layer data including all characters
    AnsiGetLayer {
        /// Layer index (0-based)
        layer: usize,
        response: SenderType<Result<types::LayerData, String>>,
    },

    /// Set a character at a specific position
    AnsiSetChar {
        /// Layer index (0-based)
        layer: usize,
        /// X position
        x: i32,
        /// Y position
        y: i32,
        /// Character to set
        ch: String,
        /// Text attribute
        attribute: types::TextAttributeInfo,
        response: SenderType<Result<(), String>>,
    },

    /// Set a palette color
    AnsiSetColor {
        /// Palette index (0-255)
        index: u8,
        /// Red component (0-255)
        r: u8,
        /// Green component (0-255)
        g: u8,
        /// Blue component (0-255)
        b: u8,
        response: SenderType<Result<(), String>>,
    },

    /// Get the current screen as ANSI or ASCII
    AnsiGetScreen {
        format: types::AnsiScreenFormat,
        response: SenderType<Result<String, String>>,
    },

    /// Get current caret position and attribute
    AnsiGetCaret { response: SenderType<Result<types::CaretInfo, String>> },

    /// Set caret position and attribute
    AnsiSetCaret {
        x: i32,
        y: i32,
        attribute: types::TextAttributeInfo,
        response: SenderType<Result<(), String>>,
    },

    /// List layers (metadata)
    AnsiListLayers {
        response: SenderType<Result<Vec<types::LayerInfo>, String>>,
    },

    /// Add a new layer after the specified layer index
    AnsiAddLayer {
        after_layer: usize,
        response: SenderType<Result<usize, String>>,
    },

    /// Delete a layer
    AnsiDeleteLayer { layer: usize, response: SenderType<Result<(), String>> },

    /// Update layer properties
    AnsiSetLayerProps {
        layer: usize,
        title: Option<String>,
        is_visible: Option<bool>,
        is_locked: Option<bool>,
        is_position_locked: Option<bool>,
        offset_x: Option<i32>,
        offset_y: Option<i32>,
        transparency: Option<u8>,
        response: SenderType<Result<(), String>>,
    },

    /// Merge a layer down into the layer below
    AnsiMergeDownLayer { layer: usize, response: SenderType<Result<(), String>> },

    /// Move a layer up/down in the layer stack
    AnsiMoveLayer {
        layer: usize,
        direction: types::LayerMoveDirection,
        response: SenderType<Result<(), String>>,
    },

    /// Resize the buffer
    AnsiResize {
        width: i32,
        height: i32,
        response: SenderType<Result<(), String>>,
    },

    /// Get a rectangular region from a layer
    AnsiGetRegion {
        layer: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        response: SenderType<Result<types::RegionData, String>>,
    },

    /// Set a rectangular region on a layer
    AnsiSetRegion {
        layer: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        chars: Vec<types::CharInfo>,
        response: SenderType<Result<(), String>>,
    },

    /// Get selection (if any)
    AnsiGetSelection {
        response: SenderType<Result<Option<types::SelectionInfo>, String>>,
    },

    /// Set selection rectangle
    AnsiSetSelection {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        response: SenderType<Result<(), String>>,
    },

    /// Clear selection
    AnsiClearSelection { response: SenderType<Result<(), String>> },

    /// Run a selection action
    AnsiSelectionAction { action: String, response: SenderType<Result<(), String>> },
}
