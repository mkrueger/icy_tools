//! Fill selection operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Fill a rectangular region with a value (on or off)
pub struct FillSelection {
    ch: char,
    old_data: Vec<Vec<bool>>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    value: bool,
}

impl FillSelection {
    pub fn new(ch: char, old_data: Vec<Vec<bool>>, x1: i32, y1: i32, x2: i32, y2: i32, value: bool) -> Self {
        Self {
            ch,
            old_data,
            x1,
            y1,
            x2,
            y2,
            value,
        }
    }
}

impl BitFontUndoOperation for FillSelection {
    fn get_description(&self) -> String {
        if self.value {
            "Fill selection".to_string()
        } else {
            "Erase selection".to_string()
        }
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.old_data.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.fill_region_internal(self.ch, self.x1, self.y1, self.x2, self.y2, self.value);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::EditPixels
    }
}
