//! Font-level operations for BitFont editor
//!
//! Operations that affect the entire font structure:
//! - Resize font dimensions
//! - Insert/delete lines (rows)
//! - Insert/delete columns
//! - Duplicate line
//! - Swap characters

use crate::Result;
use crate::bitfont::{DeleteColumn, DeleteLine, DuplicateLine, InsertColumn, InsertLine, ResizeFont, SwapChars};

use super::BitFontEditState;

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Resize
    // ═══════════════════════════════════════════════════════════════════════

    /// Resize font to new dimensions
    ///
    /// If new dimensions are larger, new pixels are initialized to off.
    /// If new dimensions are smaller, pixels outside the new bounds are clipped.
    pub fn resize_font(&mut self, new_width: i32, new_height: i32) -> Result<()> {
        if new_width == self.font_width && new_height == self.font_height {
            return Ok(());
        }

        // Collect all glyph data before resize
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(ResizeFont::new(self.font_width, self.font_height, new_width, new_height, old_glyph_data));
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Line Operations (affect all glyphs)
    // ═══════════════════════════════════════════════════════════════════════

    /// Insert line at cursor Y position
    ///
    /// Inserts a new empty row at the cursor's Y position in ALL glyphs.
    /// The font height increases by 1.
    pub fn insert_line(&mut self) -> Result<()> {
        let y_pos = self.cursor_pos.1 as usize;
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(InsertLine::new(y_pos, self.font_height, old_glyph_data));
        self.push_undo_action(op)
    }

    /// Delete line at cursor Y position
    ///
    /// Removes the row at the cursor's Y position from ALL glyphs.
    /// The font height decreases by 1. Does nothing if height would become 0.
    pub fn delete_line(&mut self) -> Result<()> {
        if self.font_height <= 1 {
            return Ok(());
        }

        let y_pos = self.cursor_pos.1 as usize;
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(DeleteLine::new(y_pos, self.font_height, old_glyph_data));
        self.push_undo_action(op)
    }

    /// Duplicate line at cursor Y position
    ///
    /// Copies the row at the cursor's Y position and inserts it below in ALL glyphs.
    /// The font height increases by 1.
    pub fn duplicate_line(&mut self) -> Result<()> {
        let y_pos = self.cursor_pos.1 as usize;
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(DuplicateLine::new(y_pos, self.font_height, old_glyph_data));
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Column Operations (affect all glyphs)
    // ═══════════════════════════════════════════════════════════════════════

    /// Insert column at cursor X position
    ///
    /// Inserts a new empty column at the cursor's X position in ALL glyphs.
    /// The font width increases by 1.
    pub fn insert_column(&mut self) -> Result<()> {
        let x_pos = self.cursor_pos.0 as usize;
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(InsertColumn::new(x_pos, self.font_width, old_glyph_data));
        self.push_undo_action(op)
    }

    /// Delete column at cursor X position
    ///
    /// Removes the column at the cursor's X position from ALL glyphs.
    /// The font width decreases by 1. Does nothing if width would become 0.
    pub fn delete_column(&mut self) -> Result<()> {
        if self.font_width <= 1 {
            return Ok(());
        }

        let x_pos = self.cursor_pos.0 as usize;
        let old_glyph_data: Vec<Vec<Vec<bool>>> = self.glyph_data.clone();

        let op = Box::new(DeleteColumn::new(x_pos, self.font_width, old_glyph_data));
        self.push_undo_action(op)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Character Swap
    // ═══════════════════════════════════════════════════════════════════════

    /// Swap pixel data between two characters
    ///
    /// Exchanges the entire glyph data between char1 and char2.
    /// Does nothing if both characters are the same.
    pub fn swap_chars(&mut self, char1: char, char2: char) -> Result<()> {
        if char1 == char2 {
            return Ok(());
        }

        let data1 = self.get_glyph_pixels(char1).clone();
        let data2 = self.get_glyph_pixels(char2).clone();

        let op = Box::new(SwapChars::new(char1, char2, data1, data2));
        self.push_undo_action(op)
    }
}
