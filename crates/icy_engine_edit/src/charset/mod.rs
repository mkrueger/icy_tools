//! CharSet (TDF Font) editing module
//!
//! Provides the model layer for TheDraw Font (TDF) editing, including:
//! - `CharSetEditState` - the main state container for TDF font editing
//! - Undo/redo operations for font editing actions
//! - Direct use of `retrofont::Glyph` and `retrofont::tdf::TdfFont`
//!
//! This module follows the same pattern as the BitFont editor's `BitFontEditState`,
//! separating model logic from UI concerns.

mod edit_state;
mod font_type;
mod tdf_font;
mod undo_operations;
mod undo_stack;

pub use edit_state::*;
pub use font_type::*;
pub use tdf_font::*;
pub use undo_operations::*;
pub use undo_stack::*;

// Re-export retrofont types for convenience
pub use retrofont::Glyph;
pub use retrofont::tdf::TdfFont;
