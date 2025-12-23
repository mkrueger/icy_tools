//! Shape drawing operations for BitFont editor
//!
//! High-level drawing operations that use the brush algorithms and support undo:
//! - Line drawing
//! - Rectangle drawing (outline and filled)
//! - Flood fill

use crate::bitfont::{brushes, BitFontUndoOp};
use crate::Result;

use super::BitFontEditState;

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Line Drawing
    // ═══════════════════════════════════════════════════════════════════════

    /// Draw a line from (x0, y0) to (x1, y1) using Bresenham's algorithm
    ///
    /// This creates a single undo operation for the entire line.
    ///
    /// # Arguments
    /// * `ch` - The character/glyph to draw on
    /// * `x0`, `y0` - Starting point coordinates
    /// * `x1`, `y1` - Ending point coordinates
    /// * `value` - Pixel value to set (true = on, false = off)
    pub fn draw_line(&mut self, ch: char, x0: i32, y0: i32, x1: i32, y1: i32, value: bool) -> Result<()> {
        let points = brushes::bresenham_line(x0, y0, x1, y1);
        let old_data = self.get_glyph_pixels(ch).clone();
        let mut new_data = old_data.clone();

        for (x, y) in points {
            if x >= 0 && x < self.font_width && y >= 0 && y < self.font_height {
                new_data[y as usize][x as usize] = value;
            }
        }

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Rectangle Drawing
    // ═══════════════════════════════════════════════════════════════════════

    /// Draw a rectangle from (x0, y0) to (x1, y1)
    ///
    /// This creates a single undo operation for the entire rectangle.
    ///
    /// # Arguments
    /// * `ch` - The character/glyph to draw on
    /// * `x0`, `y0` - First corner coordinates
    /// * `x1`, `y1` - Second corner coordinates (opposite corner)
    /// * `filled` - If true, draws a filled rectangle; if false, draws only the outline
    /// * `value` - Pixel value to set (true = on, false = off)
    pub fn draw_rectangle(&mut self, ch: char, x0: i32, y0: i32, x1: i32, y1: i32, filled: bool, value: bool) -> Result<()> {
        let points = brushes::rectangle_points(x0, y0, x1, y1, filled);
        let old_data = self.get_glyph_pixels(ch).clone();
        let mut new_data = old_data.clone();

        for (x, y) in points {
            if x >= 0 && x < self.font_width && y >= 0 && y < self.font_height {
                new_data[y as usize][x as usize] = value;
            }
        }

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Flood Fill
    // ═══════════════════════════════════════════════════════════════════════

    /// Flood fill from a starting point using 4-connected fill
    ///
    /// This creates a single undo operation for the entire fill.
    ///
    /// # Arguments
    /// * `ch` - The character/glyph to fill
    /// * `start_x`, `start_y` - Starting point coordinates
    /// * `fill_value` - Value to fill with (true = on, false = off)
    pub fn flood_fill(&mut self, ch: char, start_x: i32, start_y: i32, fill_value: bool) -> Result<()> {
        // Bounds check
        if start_x < 0 || start_x >= self.font_width || start_y < 0 || start_y >= self.font_height {
            return Ok(());
        }

        let old_data = self.get_glyph_pixels(ch).clone();

        // Get target value at start position
        let target_value = old_data
            .get(start_y as usize)
            .and_then(|row| row.get(start_x as usize))
            .copied()
            .unwrap_or(false);

        // Don't fill if we're trying to fill with the same value
        if target_value == fill_value {
            return Ok(());
        }

        // Get the points to fill
        let points = brushes::flood_fill_points(start_x, start_y, self.font_width, self.font_height, |x, y| {
            old_data.get(y as usize).and_then(|row| row.get(x as usize)).copied().unwrap_or(false)
        });

        // Apply the fill
        let mut new_data = old_data.clone();
        for (x, y) in points {
            if x >= 0 && x < self.font_width && y >= 0 && y < self.font_height {
                new_data[y as usize][x as usize] = fill_value;
            }
        }

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }
}
