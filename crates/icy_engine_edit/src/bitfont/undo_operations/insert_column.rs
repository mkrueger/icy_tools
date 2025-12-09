//! Insert column operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Insert a column at specified X position in all glyphs
pub struct InsertColumn {
    x_pos: usize,
    old_width: i32,
    old_glyph_data: Vec<Vec<Vec<bool>>>,
}

impl InsertColumn {
    pub fn new(x_pos: usize, old_width: i32, old_glyph_data: Vec<Vec<Vec<bool>>>) -> Self {
        Self {
            x_pos,
            old_width,
            old_glyph_data,
        }
    }
}

impl BitFontUndoOperation for InsertColumn {
    fn get_description(&self) -> String {
        "Insert column".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let (_, height) = state.font_size();
        state.set_font_dimensions_internal(self.old_width, height);

        for (i, glyph_data) in self.old_glyph_data.iter().enumerate() {
            if let Some(ch) = char::from_u32(i as u32) {
                state.set_glyph_pixels_internal(ch, glyph_data.clone());
            }
        }
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.insert_column_internal(self.x_pos);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Resize
    }
}
