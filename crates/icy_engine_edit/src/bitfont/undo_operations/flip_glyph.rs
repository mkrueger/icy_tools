//! Flip glyph operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Flip glyph horizontally or vertically within a selection region
pub struct FlipGlyph {
    ch: char,
    horizontal: bool,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl FlipGlyph {
    pub fn new(ch: char, horizontal: bool, x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            ch,
            horizontal,
            x1,
            y1,
            x2,
            y2,
        }
    }
}

impl BitFontUndoOperation for FlipGlyph {
    fn get_description(&self) -> String {
        if self.horizontal {
            "Flip horizontal".to_string()
        } else {
            "Flip vertical".to_string()
        }
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        // Flip is self-reversing
        self.redo(state)
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let mut data = state.get_glyph_pixels(self.ch).clone();

        let min_x = self.x1.min(self.x2) as usize;
        let max_x = self.x1.max(self.x2) as usize;
        let min_y = self.y1.min(self.y2) as usize;
        let max_y = self.y1.max(self.y2) as usize;

        if self.horizontal {
            // Flip horizontally within selection
            for y in min_y..=max_y {
                if y < data.len() {
                    let row = &mut data[y];
                    // Swap pixels from left to right within selection
                    let mut left = min_x;
                    let mut right = max_x;
                    while left < right {
                        if left < row.len() && right < row.len() {
                            row.swap(left, right);
                        }
                        left += 1;
                        right -= 1;
                    }
                }
            }
        } else {
            // Flip vertically within selection
            let mut top = min_y;
            let mut bottom = max_y;
            while top < bottom {
                if top < data.len() && bottom < data.len() {
                    // Swap the pixels within the selection columns for these rows
                    for x in min_x..=max_x {
                        if x < data[top].len() && x < data[bottom].len() {
                            let tmp = data[top][x];
                            data[top][x] = data[bottom][x];
                            data[bottom][x] = tmp;
                        }
                    }
                }
                top += 1;
                bottom -= 1;
            }
        }

        state.set_glyph_pixels_internal(self.ch, data);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Transform
    }
}
