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
mod clipboard;
mod edit_state;
mod undo_operations;
mod undo_stack;

/// Maximum allowed font height (rows per glyph)
pub const MAX_FONT_HEIGHT: i32 = 32;

/// Minimum allowed font height (rows per glyph)
pub const MIN_FONT_HEIGHT: i32 = 1;

/// Maximum allowed font width (columns per glyph)
pub const MAX_FONT_WIDTH: i32 = 8;

/// Minimum allowed font width (columns per glyph)
pub const MIN_FONT_WIDTH: i32 = 1;

pub use clipboard::*;
pub use edit_state::*;
pub use undo_operations::*;
pub use undo_stack::*;
