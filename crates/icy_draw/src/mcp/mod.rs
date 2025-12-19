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
}
