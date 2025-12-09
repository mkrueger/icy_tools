//! Edit glyph pixels operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Edit glyph pixels operation
pub struct EditGlyph {
    ch: char,
    old_data: Vec<Vec<bool>>,
    new_data: Vec<Vec<bool>>,
}

impl EditGlyph {
    pub fn new(ch: char, old_data: Vec<Vec<bool>>, new_data: Vec<Vec<bool>>) -> Self {
        Self { ch, old_data, new_data }
    }
}

impl BitFontUndoOperation for EditGlyph {
    fn get_description(&self) -> String {
        "Edit glyph".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.old_data.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.new_data.clone());
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::EditPixels
    }
}
