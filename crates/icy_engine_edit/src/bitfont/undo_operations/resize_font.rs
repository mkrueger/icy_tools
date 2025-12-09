//! Resize font operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Resize all glyphs in the font
pub struct ResizeFont {
    old_width: i32,
    old_height: i32,
    new_width: i32,
    new_height: i32,
    old_glyph_data: Vec<Vec<Vec<bool>>>,
}

impl ResizeFont {
    pub fn new(old_width: i32, old_height: i32, new_width: i32, new_height: i32, old_glyph_data: Vec<Vec<Vec<bool>>>) -> Self {
        Self {
            old_width,
            old_height,
            new_width,
            new_height,
            old_glyph_data,
        }
    }
}

impl BitFontUndoOperation for ResizeFont {
    fn get_description(&self) -> String {
        "Resize font".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        // Restore old dimensions first
        state.set_font_dimensions_internal(self.old_width, self.old_height);

        // Restore old glyph data
        for (i, glyph_data) in self.old_glyph_data.iter().enumerate() {
            if let Some(ch) = char::from_u32(i as u32) {
                state.set_glyph_pixels_internal(ch, glyph_data.clone());
            }
        }
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.resize_glyphs_internal(self.new_width, self.new_height);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Resize
    }
}
