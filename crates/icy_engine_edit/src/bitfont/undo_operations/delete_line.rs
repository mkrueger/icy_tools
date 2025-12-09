//! Delete line operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Delete a row at specified Y position from all glyphs
pub struct DeleteLine {
    y_pos: usize,
    old_height: i32,
    old_glyph_data: Vec<Vec<Vec<bool>>>,
}

impl DeleteLine {
    pub fn new(y_pos: usize, old_height: i32, old_glyph_data: Vec<Vec<Vec<bool>>>) -> Self {
        Self {
            y_pos,
            old_height,
            old_glyph_data,
        }
    }
}

impl BitFontUndoOperation for DeleteLine {
    fn get_description(&self) -> String {
        "Delete line".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let (width, _) = state.font_size();
        state.set_font_dimensions_internal(width, self.old_height);

        for (i, glyph_data) in self.old_glyph_data.iter().enumerate() {
            if let Some(ch) = char::from_u32(i as u32) {
                state.set_glyph_pixels_internal(ch, glyph_data.clone());
            }
        }
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.delete_line_internal(self.y_pos);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Resize
    }
}
