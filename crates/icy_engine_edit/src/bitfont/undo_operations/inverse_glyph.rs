//! Inverse glyph operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Inverse glyph operation (toggle all pixels)
pub struct InverseGlyph {
    ch: char,
}

impl InverseGlyph {
    pub fn new(ch: char) -> Self {
        Self { ch }
    }
}

impl BitFontUndoOperation for InverseGlyph {
    fn get_description(&self) -> String {
        "Inverse glyph".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        // Inverse is self-reversing
        self.redo(state)
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let data = state.get_glyph_pixels(self.ch).clone();
        let inverted: Vec<Vec<bool>> = data.iter().map(|row| row.iter().map(|&p| !p).collect()).collect();
        state.set_glyph_pixels_internal(self.ch, inverted);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Transform
    }
}
