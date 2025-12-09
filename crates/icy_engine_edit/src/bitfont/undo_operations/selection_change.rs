//! Selection change operation
//!
//! Tracks changes to selection state for undo/redo, including:
//! - Edit selection (pixel-level)
//! - Charset selection (character-level)
//! - Cursor positions for both panels

use icy_engine::{Position, Selection};

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Selection change operation - stores complete selection state
pub struct SelectionChange {
    // Edit selection (pixel-level)
    old_edit_selection: Option<Selection>,
    new_edit_selection: Option<Selection>,

    // Charset selection (anchor, lead, is_rectangle)
    old_charset_selection: Option<(Position, Position, bool)>,
    new_charset_selection: Option<(Position, Position, bool)>,

    // Cursor positions
    old_cursor_pos: (i32, i32),
    new_cursor_pos: (i32, i32),
    old_charset_cursor: (i32, i32),
    new_charset_cursor: (i32, i32),
}

impl SelectionChange {
    pub fn new(
        old_edit_selection: Option<Selection>,
        new_edit_selection: Option<Selection>,
        old_charset_selection: Option<(Position, Position, bool)>,
        new_charset_selection: Option<(Position, Position, bool)>,
        old_cursor_pos: (i32, i32),
        new_cursor_pos: (i32, i32),
        old_charset_cursor: (i32, i32),
        new_charset_cursor: (i32, i32),
    ) -> Self {
        Self {
            old_edit_selection,
            new_edit_selection,
            old_charset_selection,
            new_charset_selection,
            old_cursor_pos,
            new_cursor_pos,
            old_charset_cursor,
            new_charset_cursor,
        }
    }
}

impl BitFontUndoOperation for SelectionChange {
    fn get_description(&self) -> String {
        "Selection change".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_edit_selection_internal(self.old_edit_selection);
        state.set_charset_selection_internal(self.old_charset_selection);
        state.set_cursor_pos_internal(self.old_cursor_pos);
        state.set_charset_cursor_internal(self.old_charset_cursor);
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_edit_selection_internal(self.new_edit_selection);
        state.set_charset_selection_internal(self.new_charset_selection);
        state.set_cursor_pos_internal(self.new_cursor_pos);
        state.set_charset_cursor_internal(self.new_charset_cursor);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Unknown
    }

    fn changes_data(&self) -> bool {
        // Selection changes don't affect the font data itself
        false
    }
}
