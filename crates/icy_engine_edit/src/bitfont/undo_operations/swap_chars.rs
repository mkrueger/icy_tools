//! Swap characters operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Swap two characters' glyph data
pub struct SwapChars {
    char1: char,
    char2: char,
    data1: Vec<Vec<bool>>,
    data2: Vec<Vec<bool>>,
}

impl SwapChars {
    pub fn new(char1: char, char2: char, data1: Vec<Vec<bool>>, data2: Vec<Vec<bool>>) -> Self {
        Self { char1, char2, data1, data2 }
    }
}

impl BitFontUndoOperation for SwapChars {
    fn get_description(&self) -> String {
        "Swap characters".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        // Restore original data
        state.set_glyph_pixels_internal(self.char1, self.data1.clone());
        state.set_glyph_pixels_internal(self.char2, self.data2.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        // Swap: put data1 in char2 and data2 in char1
        state.set_glyph_pixels_internal(self.char1, self.data2.clone());
        state.set_glyph_pixels_internal(self.char2, self.data1.clone());
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Transform
    }
}
