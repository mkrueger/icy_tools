//! BitFont editing module
//!
//! Provides the model layer for bitmap font editing, including:
//! - `BitFontEditState` - the main state container for font editing
//! - Undo/redo operations for all font editing actions
//! - Glyph manipulation functions
//! - Brush/shape drawing algorithms (line, rectangle, flood fill)
//!
//! This module follows the same pattern as the ANSI editor's `EditState`,
//! separating model logic from UI concerns.

pub mod brushes;
pub mod clipboard;
mod edit_state;
pub mod session_state;
mod undo_operation;
mod undo_stack;

// Re-export clipboard types for convenience
pub use clipboard::{
    copy_to_clipboard, get_from_clipboard, has_bitfont_data, BitFontClipboardData, BitFontClipboardError, BITFONT_CLIPBOARD_TYPE,
};

/// Maximum allowed font height (rows per glyph)
pub const MAX_FONT_HEIGHT: i32 = 32;

/// Minimum allowed font height (rows per glyph)
pub const MIN_FONT_HEIGHT: i32 = 1;

/// Maximum allowed font width (columns per glyph)
pub const MAX_FONT_WIDTH: i32 = 8;

/// Minimum allowed font width (columns per glyph)
pub const MIN_FONT_WIDTH: i32 = 1;

pub use edit_state::*;
pub use session_state::{BitFontFocusedPanelState, BitFontSessionState};
pub use undo_operation::{BitFontOperationType, BitFontUndoOp};
pub use undo_stack::{BitFontUndoStack, BitFontUndoState};
