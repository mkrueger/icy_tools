//! Inverse selection operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Inverse a rectangular region
pub struct InverseSelection {
    ch: char,
    old_data: Vec<Vec<bool>>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl InverseSelection {
    pub fn new(ch: char, old_data: Vec<Vec<bool>>, x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { ch, old_data, x1, y1, x2, y2 }
    }
}

impl BitFontUndoOperation for InverseSelection {
    fn get_description(&self) -> String {
        "Inverse selection".to_string()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.old_data.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.inverse_region_internal(self.ch, self.x1, self.y1, self.x2, self.y2);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Transform
    }
}
