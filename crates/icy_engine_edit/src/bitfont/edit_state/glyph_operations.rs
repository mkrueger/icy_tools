//! Glyph-level operations for BitFont editor
//!
//! Operations that work on individual glyphs:
//! - Pixel editing (set_pixel, toggle_pixel)
//! - Clear glyph
//! - Flip (horizontal/vertical)
//! - Inverse
//! - Move (with clip)
//! - Slide (with wrap)
//!
//! For context-sensitive multi-glyph operations, see `charset_operations.rs`.

use icy_engine::Selection;

use crate::bitfont::BitFontUndoOp;
use crate::Result;

use super::{BitFontEditState, BitFontFocusedPanel};

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Pixel Editing
    // ═══════════════════════════════════════════════════════════════════════

    /// Set a single pixel value
    pub fn set_pixel(&mut self, ch: char, x: i32, y: i32, value: bool) -> Result<()> {
        if x < 0 || x >= self.font_width || y < 0 || y >= self.font_height {
            return Ok(());
        }

        let old_data = self.get_glyph_pixels(ch).clone();
        let mut new_data = old_data.clone();
        new_data[y as usize][x as usize] = value;

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    /// Toggle a single pixel (flip its value)
    pub fn toggle_pixel(&mut self, ch: char, x: i32, y: i32) -> Result<()> {
        if x < 0 || x >= self.font_width || y < 0 || y >= self.font_height {
            return Ok(());
        }

        let old_data = self.get_glyph_pixels(ch).clone();
        let current = old_data[y as usize][x as usize];
        let mut new_data = old_data.clone();
        new_data[y as usize][x as usize] = !current;

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    /// Set glyph pixels (with undo)
    pub fn set_glyph_pixels(&mut self, ch: char, new_data: Vec<Vec<bool>>) -> Result<()> {
        let old_data = self.get_glyph_pixels(ch).clone();
        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Clear
    // ═══════════════════════════════════════════════════════════════════════

    /// Clear glyph (set all pixels to off)
    ///
    /// Low-level operation that always clears the entire glyph.
    /// For context-sensitive clearing (respecting selection and focus), use `erase_selection()`.
    pub fn clear_glyph(&mut self, ch: char) -> Result<()> {
        let old_data = self.get_glyph_pixels(ch).clone();
        let op = BitFontUndoOp::ClearGlyph { ch, old_data };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Flip
    // ═══════════════════════════════════════════════════════════════════════

    /// Flip glyph horizontally (mirror along vertical axis)
    ///
    /// **With edit_selection**: Flips only the selected rectangle, pixels outside unchanged.
    /// **Without selection**: Flips the entire glyph.
    ///
    /// Note: This is a low-level operation on a single glyph. The UI layer may call this
    /// for each glyph in a charset selection.
    pub fn flip_glyph_x(&mut self, ch: char) -> Result<()> {
        let selection = self.get_edit_selection_or_all();
        let (x1, y1) = (selection.anchor.x, selection.anchor.y);
        let (x2, y2) = (selection.lead.x, selection.lead.y);

        let op = BitFontUndoOp::FlipGlyph {
            ch,
            horizontal: true,
            x1,
            y1,
            x2,
            y2,
        };
        self.push_undo_action(op)
    }

    /// Flip glyph vertically (mirror along horizontal axis)
    ///
    /// **With edit_selection**: Flips only the selected rectangle, pixels outside unchanged.
    /// **Without selection**: Flips the entire glyph.
    ///
    /// Note: This is a low-level operation on a single glyph. The UI layer may call this
    /// for each glyph in a charset selection.
    pub fn flip_glyph_y(&mut self, ch: char) -> Result<()> {
        let selection = self.get_edit_selection_or_all();
        let (x1, y1) = (selection.anchor.x, selection.anchor.y);
        let (x2, y2) = (selection.lead.x, selection.lead.y);

        let op = BitFontUndoOp::FlipGlyph {
            ch,
            horizontal: false,
            x1,
            y1,
            x2,
            y2,
        };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Inverse
    // ═══════════════════════════════════════════════════════════════════════

    /// Inverse all pixels in a single glyph
    ///
    /// Low-level operation. For context-sensitive inverse, use `inverse_edit_selection()`.
    pub fn inverse_glyph(&mut self, ch: char) -> Result<()> {
        let op = BitFontUndoOp::InverseGlyph { ch };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Move (with clip)
    // ═══════════════════════════════════════════════════════════════════════

    /// Move glyph pixels by offset (with clip, not wrap)
    ///
    /// Shifts all pixels in the glyph by (dx, dy). Pixels that fall outside
    /// the glyph boundaries are clipped (lost). This is different from `slide_glyph()`
    /// which wraps pixels around.
    pub fn move_glyph(&mut self, ch: char, dx: i32, dy: i32) -> Result<()> {
        let old_data = self.get_glyph_pixels(ch).clone();
        let op = BitFontUndoOp::MoveGlyph { ch, dx, dy, old_data };
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Slide (with wrap)
    // ═══════════════════════════════════════════════════════════════════════

    /// Slide glyph pixels (rotate with wrap-around)
    ///
    /// Unlike `move_glyph()` which clips pixels at boundaries, slide wraps them
    /// around to the opposite side.
    ///
    /// **EditGrid focus**: Slides pixels within the edit_selection (or entire glyph).
    /// **CharSet focus with multiple chars**: Treats all selected glyphs as one combined
    /// region - pixels sliding off one glyph appear on the adjacent glyph.
    ///
    /// dx/dy: direction to slide (-1 for left/up, +1 for right/down)
    pub fn slide_glyph(&mut self, dx: i32, dy: i32) -> Result<()> {
        let target_chars = self.get_target_chars();
        let selection = self.get_edit_selection_or_all();

        let description = if dx != 0 {
            if dx > 0 {
                "Slide right"
            } else {
                "Slide left"
            }
        } else if dy > 0 {
            "Slide down"
        } else {
            "Slide up"
        };

        if self.focused_panel == BitFontFocusedPanel::CharSet && target_chars.len() > 1 {
            // CharSet mode with multiple chars: slide across all chars as one combined region
            self.slide_across_chars(&target_chars, &selection, dx, dy, description)
        } else {
            // Single char mode: slide within the char
            let mut guard = self.begin_atomic_undo(description);

            for ch in target_chars {
                let old_data = self.get_glyph_pixels(ch).clone();
                let new_data = self.slide_pixels_single(&old_data, &selection, dx, dy);
                let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
                self.push_undo_action(op)?;
            }

            self.end_atomic_undo(guard.base_count(), guard.description().to_string(), guard.operation_type());
            guard.mark_ended();
            Ok(())
        }
    }

    /// Slide pixels across multiple characters as one combined region
    /// Pixels that fall off one char wrap to the next char in selection order
    fn slide_across_chars(&mut self, chars: &[char], selection: &Selection, dx: i32, dy: i32, description: &str) -> Result<()> {
        if chars.is_empty() {
            return Ok(());
        }

        let min_x = selection.anchor.x.min(selection.lead.x).max(0) as usize;
        let max_x = selection.anchor.x.max(selection.lead.x).min(self.font_width - 1) as usize;
        let min_y = selection.anchor.y.min(selection.lead.y).max(0) as usize;
        let max_y = selection.anchor.y.max(selection.lead.y).min(self.font_height - 1) as usize;

        let sel_width = max_x - min_x + 1;
        let sel_height = max_y - min_y + 1;

        if sel_width == 0 || sel_height == 0 {
            return Ok(());
        }

        let num_chars = chars.len();

        // Build a combined region: all chars' selection regions concatenated
        // For horizontal slide: treat as one wide row per y-level
        // For vertical slide: treat as one tall column per x-level

        // Collect all glyph data
        let old_data: Vec<Vec<Vec<bool>>> = chars.iter().map(|&ch| self.get_glyph_pixels(ch).clone()).collect();

        let mut new_data = old_data.clone();

        if dx != 0 {
            // Horizontal slide: for each row, concatenate all chars' pixels and rotate
            for y in min_y..=max_y {
                // Build combined row across all chars
                let mut combined_row: Vec<bool> = Vec::with_capacity(sel_width * num_chars);
                for char_data in &old_data {
                    for x in min_x..=max_x {
                        combined_row.push(char_data.get(y).and_then(|r| r.get(x)).copied().unwrap_or(false));
                    }
                }

                // Rotate the combined row
                let len = combined_row.len();
                if dx > 0 {
                    combined_row.rotate_right(dx.unsigned_abs() as usize % len);
                } else {
                    combined_row.rotate_left((-dx) as usize % len);
                }

                // Write back to individual chars
                let mut idx = 0;
                for (char_idx, _) in chars.iter().enumerate() {
                    for x in min_x..=max_x {
                        if y < new_data[char_idx].len() && x < new_data[char_idx][y].len() {
                            new_data[char_idx][y][x] = combined_row[idx];
                        }
                        idx += 1;
                    }
                }
            }
        }

        if dy != 0 {
            // Vertical slide: for each column, concatenate all chars' pixels and rotate
            for x in min_x..=max_x {
                // Build combined column across all chars
                let mut combined_col: Vec<bool> = Vec::with_capacity(sel_height * num_chars);
                for char_data in &old_data {
                    for y in min_y..=max_y {
                        combined_col.push(char_data.get(y).and_then(|r| r.get(x)).copied().unwrap_or(false));
                    }
                }

                // Rotate the combined column
                let len = combined_col.len();
                if dy > 0 {
                    combined_col.rotate_right(dy.unsigned_abs() as usize % len);
                } else {
                    combined_col.rotate_left((-dy) as usize % len);
                }

                // Write back to individual chars
                let mut idx = 0;
                for (char_idx, _) in chars.iter().enumerate() {
                    for y in min_y..=max_y {
                        if y < new_data[char_idx].len() && x < new_data[char_idx][y].len() {
                            new_data[char_idx][y][x] = combined_col[idx];
                        }
                        idx += 1;
                    }
                }
            }
        }

        // Create undo operations
        let mut guard = self.begin_atomic_undo(description);

        for (i, &ch) in chars.iter().enumerate() {
            let op = BitFontUndoOp::EditGlyph {
                ch,
                old_data: old_data[i].clone(),
                new_data: new_data[i].clone(),
            };
            self.push_undo_action(op)?;
        }

        self.end_atomic_undo(guard.base_count(), guard.description().to_string(), guard.operation_type());
        guard.mark_ended();
        Ok(())
    }

    /// Helper: slide pixels within a selection region with wrap-around (single char)
    fn slide_pixels_single(&self, data: &[Vec<bool>], selection: &Selection, dx: i32, dy: i32) -> Vec<Vec<bool>> {
        let mut result = data.to_vec();

        let min_x = selection.anchor.x.min(selection.lead.x).max(0) as usize;
        let max_x = selection.anchor.x.max(selection.lead.x).min(self.font_width - 1) as usize;
        let min_y = selection.anchor.y.min(selection.lead.y).max(0) as usize;
        let max_y = selection.anchor.y.max(selection.lead.y).min(self.font_height - 1) as usize;

        let sel_width = max_x - min_x + 1;
        let sel_height = max_y - min_y + 1;

        if sel_width == 0 || sel_height == 0 {
            return result;
        }

        // Extract the selection region
        let mut region: Vec<Vec<bool>> = Vec::with_capacity(sel_height);
        for y in min_y..=max_y {
            let mut row = Vec::with_capacity(sel_width);
            for x in min_x..=max_x {
                row.push(data.get(y).and_then(|r| r.get(x)).copied().unwrap_or(false));
            }
            region.push(row);
        }

        // Slide horizontally
        if dx != 0 {
            for row in &mut region {
                if dx > 0 {
                    // Slide right: last element wraps to first
                    row.rotate_right(dx.unsigned_abs() as usize % sel_width);
                } else {
                    // Slide left: first element wraps to last
                    row.rotate_left((-dx) as usize % sel_width);
                }
            }
        }

        // Slide vertically
        if dy != 0 {
            if dy > 0 {
                // Slide down: last row wraps to first
                region.rotate_right(dy.unsigned_abs() as usize % sel_height);
            } else {
                // Slide up: first row wraps to last
                region.rotate_left((-dy) as usize % sel_height);
            }
        }

        // Write the region back
        for (ry, y) in (min_y..=max_y).enumerate() {
            for (rx, x) in (min_x..=max_x).enumerate() {
                if y < result.len() && x < result[y].len() {
                    result[y][x] = region[ry][rx];
                }
            }
        }

        result
    }
}
