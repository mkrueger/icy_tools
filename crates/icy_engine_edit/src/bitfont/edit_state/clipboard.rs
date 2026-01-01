//! Clipboard operations for BitFont editor
//!
//! Copy, cut, and paste operations for pixel data.
//!
//! Clipboard format is custom BitFont data that preserves pixel data dimensions.
//!
//! All clipboard operations return Tasks that must be executed by the icy_ui runtime.

use crate::bitfont::BitFontUndoOp;
use crate::Result;
use icy_ui::Task;

use super::{BitFontEditState, BitFontFocusedPanel};

impl BitFontEditState {
    /// Get pixel data to copy (selection or entire glyph)
    fn get_copy_data(&self) -> Vec<Vec<bool>> {
        let glyph_data = self.get_glyph_pixels(self.selected_char);

        if let Some(selection) = &self.edit_selection {
            // Copy selection region
            let min_x = selection.anchor.x.min(selection.lead.x).max(0) as usize;
            let max_x = selection.anchor.x.max(selection.lead.x).min(self.font_width - 1) as usize;
            let min_y = selection.anchor.y.min(selection.lead.y).max(0) as usize;
            let max_y = selection.anchor.y.max(selection.lead.y).min(self.font_height - 1) as usize;

            let mut result = Vec::new();
            for y in min_y..=max_y {
                let mut row = Vec::new();
                for x in min_x..=max_x {
                    row.push(glyph_data.get(y).and_then(|r| r.get(x)).copied().unwrap_or(false));
                }
                result.push(row);
            }
            result
        } else {
            // Copy entire glyph
            glyph_data.clone()
        }
    }

    /// Copy selection (or entire glyph if no selection) to clipboard
    ///
    /// Returns a Task that performs the clipboard write.
    /// Clears edit and charset selections after copying.
    pub fn copy<Message: Send + 'static>(
        &mut self,
        on_complete: impl Fn(std::result::Result<(), crate::bitfont::BitFontClipboardError>) -> Message + Send + 'static,
    ) -> Task<Message> {
        use crate::bitfont::{copy_to_clipboard, BitFontClipboardData};

        let pixels = self.get_copy_data();
        let data = BitFontClipboardData::new(pixels);

        // Clear selections after initiating copy
        self.clear_edit_selection();
        self.clear_charset_selection();

        copy_to_clipboard(&data, on_complete)
    }

    /// Cut selection (or entire glyph if no selection) to clipboard
    ///
    /// Returns a Task that performs the clipboard write.
    /// Copies to clipboard then erases the selection.
    /// Clears edit and charset selections after cutting.
    pub fn cut<Message: Send + 'static>(
        &mut self,
        on_complete: impl Fn(std::result::Result<(), crate::bitfont::BitFontClipboardError>) -> Message + Send + 'static,
    ) -> Task<Message> {
        use crate::bitfont::{copy_to_clipboard, BitFontClipboardData};

        // Copy data to clipboard first (before erasing)
        let pixels = self.get_copy_data();
        let data = BitFontClipboardData::new(pixels);

        // Then erase the selection (or entire glyph)
        let _ = self.erase_selection();

        // Clear selections after successful cut
        self.clear_edit_selection();
        self.clear_charset_selection();

        copy_to_clipboard(&data, on_complete)
    }

    /// Initiate paste from clipboard
    ///
    /// Returns a Task that reads the clipboard and calls the callback with the result.
    /// The callback receives the parsed BitFontClipboardData or an error.
    pub fn paste<Message: Send + 'static>(
        on_result: impl Fn(std::result::Result<crate::bitfont::BitFontClipboardData, crate::bitfont::BitFontClipboardError>) -> Message + Send + 'static,
    ) -> Task<Message> {
        crate::bitfont::get_from_clipboard(on_result)
    }

    /// Apply pasted data to the editor
    ///
    /// This should be called with the result from the paste Task callback.
    pub fn paste_data(&mut self, clipboard_data: crate::bitfont::BitFontClipboardData) -> Result<()> {
        let target_chars = self.get_target_chars();

        if self.focused_panel == BitFontFocusedPanel::CharSet {
            // In CharSet mode: paste to all target chars at top-left (0, 0)
            let guard = self.begin_atomic_undo("Paste");
            let base_count = guard.base_count();

            for ch in target_chars {
                self.paste_data_at(ch, 0, 0, &clipboard_data.pixels)?;
            }

            self.end_atomic_undo(base_count, "Paste".to_string(), crate::bitfont::BitFontOperationType::Unknown);
            Ok(())
        } else {
            // In EditGrid mode: paste at cursor position
            let (cursor_x, cursor_y) = self.cursor_pos;
            let ch = self.selected_char;

            self.paste_data_at(ch, cursor_x, cursor_y, &clipboard_data.pixels)
        }
    }

    /// Paste clipboard data at specified position in a glyph
    fn paste_data_at(&mut self, ch: char, x: i32, y: i32, pixels: &[Vec<bool>]) -> Result<()> {
        let old_data = self.get_glyph_pixels(ch).clone();
        let mut new_data = old_data.clone();

        let clip_height = pixels.len();
        let clip_width = if clip_height > 0 { pixels[0].len() } else { 0 };

        // Calculate actual paste region (clip to glyph bounds)
        let paste_x_start = x.max(0) as usize;
        let paste_y_start = y.max(0) as usize;
        let paste_x_end = ((x + clip_width as i32) as usize).min(self.font_width as usize);
        let paste_y_end = ((y + clip_height as i32) as usize).min(self.font_height as usize);

        // Calculate offset into clipboard data (for when x/y are negative)
        let clip_x_offset = if x < 0 { (-x) as usize } else { 0 };
        let clip_y_offset = if y < 0 { (-y) as usize } else { 0 };

        // Paste the pixels
        for dest_y in paste_y_start..paste_y_end {
            let src_y = dest_y - paste_y_start + clip_y_offset;
            if src_y >= clip_height {
                break;
            }

            for dest_x in paste_x_start..paste_x_end {
                let src_x = dest_x - paste_x_start + clip_x_offset;
                if src_x >= clip_width {
                    break;
                }

                if dest_y < new_data.len() && dest_x < new_data[dest_y].len() {
                    new_data[dest_y][dest_x] = pixels[src_y][src_x];
                }
            }
        }

        let op = BitFontUndoOp::EditGlyph { ch, old_data, new_data };
        self.push_undo_action(op)
    }

    /// Check if clipboard contains BitFont data
    ///
    /// Returns a Task that checks clipboard availability.
    pub fn can_paste<Message: Send + 'static>(on_result: impl Fn(bool) -> Message + Send + 'static) -> Task<Message> {
        crate::bitfont::has_bitfont_data(on_result)
    }
}
