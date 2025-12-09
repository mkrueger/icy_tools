//! Cursor movement operations for BitFont editor
//!
//! Handles cursor movement in both the edit grid and charset grid.
//! Both cursors wrap at boundaries using `rem_euclid`.

use super::BitFontEditState;

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Edit Grid Cursor
    // ═══════════════════════════════════════════════════════════════════════

    /// Get cursor position in edit grid
    pub fn cursor_pos(&self) -> (i32, i32) {
        self.cursor_pos
    }

    /// Set cursor position in edit grid (clamps to valid range)
    pub fn set_cursor_pos(&mut self, x: i32, y: i32) {
        self.cursor_pos = (x.clamp(0, self.font_width - 1), y.clamp(0, self.font_height - 1));
    }

    /// Move cursor by delta with wrapping at boundaries
    ///
    /// X and Y wrap independently:
    /// - Moving right from last column wraps to column 0 (same row)
    /// - Moving down from last row wraps to row 0 (same column)
    /// - Moving left from column 0 wraps to last column (same row)
    /// - Moving up from row 0 wraps to last row (same column)
    ///
    /// Note: Use `set_cursor_pos()` if you want clamping instead of wrapping.
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        let (x, y) = self.cursor_pos;

        // Wrap X and Y independently using rem_euclid for proper negative handling
        let new_x = (x + dx).rem_euclid(self.font_width);
        let new_y = (y + dy).rem_euclid(self.font_height);

        self.cursor_pos = (new_x, new_y);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Charset Grid Cursor
    // ═══════════════════════════════════════════════════════════════════════

    /// Get charset cursor position (0-15, 0-15 for 16x16 grid)
    pub fn charset_cursor(&self) -> (i32, i32) {
        self.charset_cursor
    }

    /// Set charset cursor position (clamps to valid range 0-15)
    pub fn set_charset_cursor(&mut self, x: i32, y: i32) {
        self.charset_cursor = (x.clamp(0, 15), y.clamp(0, 15));
    }

    /// Move charset cursor by delta with wrapping at boundaries
    ///
    /// X and Y wrap independently within 16x16 grid:
    /// - Moving right from column 15 wraps to column 0 (same row)
    /// - Moving down from row 15 wraps to row 0 (same column)
    /// - Moving left from column 0 wraps to column 15 (same row)
    /// - Moving up from row 0 wraps to row 15 (same column)
    ///
    /// Note: Use `set_charset_cursor()` if you want clamping instead of wrapping.
    pub fn move_charset_cursor(&mut self, dx: i32, dy: i32) {
        let (x, y) = self.charset_cursor;

        // Wrap X and Y independently using rem_euclid for proper negative handling
        let new_x = (x + dx).rem_euclid(16);
        let new_y = (y + dy).rem_euclid(16);

        self.charset_cursor = (new_x, new_y);
    }

    /// Get character at charset cursor position
    pub fn char_at_charset_cursor(&self) -> char {
        let (x, y) = self.charset_cursor;
        char::from_u32((y * 16 + x) as u32).unwrap_or(' ')
    }

    /// Select character at charset cursor (sets selected_char to cursor position)
    pub fn select_char_at_cursor(&mut self) {
        self.selected_char = self.char_at_charset_cursor();
    }
}
