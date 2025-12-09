//! Clear glyph operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontUndoOperation};

/// Clear glyph operation
pub struct ClearGlyph {
    ch: char,
    old_data: Vec<Vec<bool>>,
}

impl ClearGlyph {
    pub fn new(ch: char, old_data: Vec<Vec<bool>>) -> Self {
        Self { ch, old_data }
    }
}

impl BitFontUndoOperation for ClearGlyph {
    fn get_description(&self) -> String {
        "Clear glyph".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.old_data.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let (width, height) = state.font_size();
        let cleared = vec![vec![false; width as usize]; height as usize];
        state.set_glyph_pixels_internal(self.ch, cleared);
        Ok(())
    }
}
