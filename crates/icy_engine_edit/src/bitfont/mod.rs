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

pub use clipboard::*;
pub use edit_state::*;
pub use undo_operations::*;
pub use undo_stack::*;
