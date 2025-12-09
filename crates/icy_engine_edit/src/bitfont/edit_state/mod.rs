//! BitFont edit state module structure
//!
//! Split into multiple files matching the test structure:
//! - `state.rs` - Struct definition, constructors, getters, basic setters
//! - `cursor.rs` - Cursor movement (edit grid and charset)
//! - `selection.rs` - Selection handling (edit and charset)
//! - `glyph_operations.rs` - Single glyph operations (pixel, clear, flip, inverse, move, slide)
//! - `charset_operations.rs` - Context-sensitive multi-glyph operations (fill, erase, inverse)
//! - `shape_operations.rs` - Shape drawing (line, rectangle, flood fill)
//! - `font_operations.rs` - Font-level operations (resize, insert/delete line/column)
//! - `clipboard.rs` - Copy, cut, paste
//! - `undo.rs` - Undo/redo system
//! - `internal.rs` - Internal setters for undo operations

mod atomic_undo_guard;
mod focused_panel;
mod preview;

// State struct and basic operations
mod state;

// Implementation split by category (matching test structure)
mod charset_operations;
mod clipboard;
mod cursor;
mod font_operations;
mod glyph_operations;
mod internal;
mod selection;
mod shape_operations;
mod undo;

pub use atomic_undo_guard::BitFontAtomicUndoGuard;
pub use focused_panel::BitFontFocusedPanel;
pub use state::BitFontEditState;
