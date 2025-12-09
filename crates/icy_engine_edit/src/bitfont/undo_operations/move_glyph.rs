//! Move glyph operation

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Move glyph pixels by offset
pub struct MoveGlyph {
    ch: char,
    dx: i32,
    dy: i32,
    old_data: Vec<Vec<bool>>,
}

impl MoveGlyph {
    pub fn new(ch: char, dx: i32, dy: i32, old_data: Vec<Vec<bool>>) -> Self {
        Self { ch, dx, dy, old_data }
    }
}

impl BitFontUndoOperation for MoveGlyph {
    fn get_description(&self) -> String {
        match (self.dx, self.dy) {
            (0, -1) => "Move up".to_string(),
            (0, 1) => "Move down".to_string(),
            (-1, 0) => "Move left".to_string(),
            (1, 0) => "Move right".to_string(),
            _ => "Move glyph".to_string(),
        }
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        state.set_glyph_pixels_internal(self.ch, self.old_data.clone());
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let (width, height) = state.font_size();
        let mut new_data = vec![vec![false; width as usize]; height as usize];

        for y in 0..height as usize {
            for x in 0..width as usize {
                // Wrap source coordinates using rem_euclid for proper negative handling
                let src_x = (x as i32 - self.dx).rem_euclid(width) as usize;
                let src_y = (y as i32 - self.dy).rem_euclid(height) as usize;

                if let Some(row) = self.old_data.get(src_y) {
                    if let Some(&pixel) = row.get(src_x) {
                        new_data[y][x] = pixel;
                    }
                }
            }
        }

        state.set_glyph_pixels_internal(self.ch, new_data);
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Transform
    }
}
