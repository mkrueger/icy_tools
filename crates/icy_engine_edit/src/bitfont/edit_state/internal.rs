//! Internal setters for BitFont editor
//!
//! These methods are called by undo operations to directly modify state
//! without creating new undo entries. They should not be called directly
//! by user code.

use icy_engine::{Position, Selection};

use super::BitFontEditState;

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Selection Internal Setters
    // ═══════════════════════════════════════════════════════════════════════

    /// Set edit selection (internal, no undo)
    pub(crate) fn set_edit_selection_internal(&mut self, sel: Option<Selection>) {
        self.edit_selection = sel;
    }

    /// Set charset selection (internal, no undo)
    pub(crate) fn set_charset_selection_internal(&mut self, sel: Option<(Position, Position, bool)>) {
        self.charset_selection = sel;
    }

    /// Set cursor position (internal, no undo)
    pub(crate) fn set_cursor_pos_internal(&mut self, pos: (i32, i32)) {
        self.cursor_pos = pos;
    }

    /// Set charset cursor (internal, no undo)
    pub(crate) fn set_charset_cursor_internal(&mut self, pos: (i32, i32)) {
        self.charset_cursor = pos;
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Glyph Internal Setters
    // ═══════════════════════════════════════════════════════════════════════

    /// Set glyph pixels (internal, no undo)
    ///
    /// Called by undo operations to restore glyph state.
    pub(crate) fn set_glyph_pixels_internal(&mut self, ch: char, data: Vec<Vec<bool>>) {
        let idx = (ch as u32).min(255) as usize;
        self.glyph_data[idx] = data;
        self.is_dirty = true;
    }

    /// Fill a rectangular region with a value (internal, no undo)
    pub(crate) fn fill_region_internal(&mut self, ch: char, x1: i32, y1: i32, x2: i32, y2: i32, value: bool) {
        let idx = (ch as u32).min(255) as usize;
        let min_x = x1.min(x2).max(0) as usize;
        let max_x = x1.max(x2).min(self.font_width - 1) as usize;
        let min_y = y1.min(y2).max(0) as usize;
        let max_y = y1.max(y2).min(self.font_height - 1) as usize;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if y < self.glyph_data[idx].len() && x < self.glyph_data[idx][y].len() {
                    self.glyph_data[idx][y][x] = value;
                }
            }
        }
        self.is_dirty = true;
    }

    /// Inverse a rectangular region (internal, no undo)
    pub(crate) fn inverse_region_internal(&mut self, ch: char, x1: i32, y1: i32, x2: i32, y2: i32) {
        let idx = (ch as u32).min(255) as usize;
        let min_x = x1.min(x2).max(0) as usize;
        let max_x = x1.max(x2).min(self.font_width - 1) as usize;
        let min_y = y1.min(y2).max(0) as usize;
        let max_y = y1.max(y2).min(self.font_height - 1) as usize;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if y < self.glyph_data[idx].len() && x < self.glyph_data[idx][y].len() {
                    self.glyph_data[idx][y][x] = !self.glyph_data[idx][y][x];
                }
            }
        }
        self.is_dirty = true;
    }

    /// Set font dimensions (internal, no undo)
    pub(crate) fn set_font_dimensions_internal(&mut self, width: i32, height: i32) {
        self.font_width = width;
        self.font_height = height;
        self.is_dirty = true;
    }

    /// Resize all glyphs (internal, no undo)
    pub(crate) fn resize_glyphs_internal(&mut self, new_width: i32, new_height: i32) {
        for glyph in &mut self.glyph_data {
            let mut new_glyph = vec![vec![false; new_width as usize]; new_height as usize];

            for (y, row) in glyph.iter().enumerate() {
                if y >= new_height as usize {
                    break;
                }
                for (x, &pixel) in row.iter().enumerate() {
                    if x >= new_width as usize {
                        break;
                    }
                    new_glyph[y][x] = pixel;
                }
            }

            *glyph = new_glyph;
        }

        self.font_width = new_width;
        self.font_height = new_height;
        self.is_dirty = true;

        // Clamp cursor
        self.cursor_pos = (self.cursor_pos.0.min(new_width - 1), self.cursor_pos.1.min(new_height - 1));
    }

    /// Insert line at position (internal, no undo)
    pub(crate) fn insert_line_internal(&mut self, y_pos: usize) {
        let new_height = self.font_height + 1;

        for glyph in &mut self.glyph_data {
            let new_row = vec![false; self.font_width as usize];
            if y_pos <= glyph.len() {
                glyph.insert(y_pos, new_row);
            } else {
                glyph.push(new_row);
            }
        }

        self.font_height = new_height;
        self.is_dirty = true;
    }

    /// Delete line at position (internal, no undo)
    pub(crate) fn delete_line_internal(&mut self, y_pos: usize) {
        if self.font_height <= 1 {
            return;
        }

        for glyph in &mut self.glyph_data {
            if y_pos < glyph.len() {
                glyph.remove(y_pos);
            }
        }

        self.font_height -= 1;
        self.is_dirty = true;

        // Clamp cursor
        if self.cursor_pos.1 >= self.font_height {
            self.cursor_pos.1 = self.font_height - 1;
        }
    }

    /// Duplicate line at position (internal, no undo)
    pub(crate) fn duplicate_line_internal(&mut self, y_pos: usize) {
        let new_height = self.font_height + 1;

        for glyph in &mut self.glyph_data {
            if y_pos < glyph.len() {
                let row_copy = glyph[y_pos].clone();
                glyph.insert(y_pos + 1, row_copy);
            }
        }

        self.font_height = new_height;
        self.is_dirty = true;
    }

    /// Insert column at position (internal, no undo)
    pub(crate) fn insert_column_internal(&mut self, x_pos: usize) {
        let new_width = self.font_width + 1;

        for glyph in &mut self.glyph_data {
            for row in glyph.iter_mut() {
                if x_pos <= row.len() {
                    row.insert(x_pos, false);
                } else {
                    row.push(false);
                }
            }
        }

        self.font_width = new_width;
        self.is_dirty = true;
    }

    /// Delete column at position (internal, no undo)
    pub(crate) fn delete_column_internal(&mut self, x_pos: usize) {
        if self.font_width <= 1 {
            return;
        }

        for glyph in &mut self.glyph_data {
            for row in glyph.iter_mut() {
                if x_pos < row.len() {
                    row.remove(x_pos);
                }
            }
        }

        self.font_width -= 1;
        self.is_dirty = true;

        // Clamp cursor
        if self.cursor_pos.0 >= self.font_width {
            self.cursor_pos.0 = self.font_width - 1;
        }
    }
}
