//! Clipboard operations for BitFont editor
//!
//! Copy, cut, and paste operations for pixel data.
//!
//! Clipboard format is custom BitFont data that preserves pixel data dimensions.

use crate::Result;
use crate::bitfont::EditGlyph;

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
    /// Clears edit and charset selections after copying.
    pub fn copy(&mut self) -> Result<()> {
        use crate::bitfont::{BitFontClipboardData, BitFontClipboardError, copy_to_clipboard};

        let pixels = self.get_copy_data();
        let data = BitFontClipboardData::new(pixels);

        copy_to_clipboard(&data).map_err(|e| {
            crate::EngineError::Generic(match e {
                BitFontClipboardError::ClipboardContextFailed(msg) => format!("Clipboard error: {}", msg),
                BitFontClipboardError::ClipboardSetFailed(msg) => format!("Failed to copy: {}", msg),
                _ => "Copy failed".to_string(),
            })
        })?;

        // Clear selections after successful copy
        self.clear_edit_selection();
        self.clear_charset_selection();

        Ok(())
    }

    /// Cut selection (or entire glyph if no selection) to clipboard
    ///
    /// Copies to clipboard then erases the selection.
    /// Clears edit and charset selections after cutting.
    pub fn cut(&mut self) -> Result<()> {
        use crate::bitfont::{BitFontClipboardData, BitFontClipboardError, copy_to_clipboard};

        // Copy data to clipboard first (before erasing)
        let pixels = self.get_copy_data();
        let data = BitFontClipboardData::new(pixels);

        copy_to_clipboard(&data).map_err(|e| {
            crate::EngineError::Generic(match e {
                BitFontClipboardError::ClipboardContextFailed(msg) => format!("Clipboard error: {}", msg),
                BitFontClipboardError::ClipboardSetFailed(msg) => format!("Failed to cut: {}", msg),
                _ => "Cut failed".to_string(),
            })
        })?;

        // Then erase the selection (or entire glyph)
        self.erase_selection()?;

        // Clear selections after successful cut
        self.clear_edit_selection();
        self.clear_charset_selection();

        Ok(())
    }

    /// Paste from clipboard at cursor position
    ///
    /// **EditGrid focus**: Pastes at cursor position, clipping if necessary.
    /// **CharSet focus**: Pastes to all selected characters at top-left (0, 0).
    pub fn paste(&mut self) -> Result<()> {
        use crate::bitfont::{BitFontClipboardError, get_from_clipboard};

        let clipboard_data = get_from_clipboard().map_err(|e| {
            crate::EngineError::Generic(match e {
                BitFontClipboardError::ClipboardContextFailed(msg) => format!("Clipboard error: {}", msg),
                BitFontClipboardError::ClipboardGetFailed(msg) => format!("Failed to paste: {}", msg),
                BitFontClipboardError::InvalidFormat => "Invalid clipboard format".to_string(),
                BitFontClipboardError::NoBitFontData => "No BitFont data in clipboard".to_string(),
                _ => "Paste failed".to_string(),
            })
        })?;

        let target_chars = self.get_target_chars();

        if self.focused_panel == BitFontFocusedPanel::CharSet {
            // In CharSet mode: paste to all target chars at top-left (0, 0)
            let mut guard = self.begin_atomic_undo("Paste");

            for ch in target_chars {
                self.paste_data_at(ch, 0, 0, &clipboard_data.pixels)?;
            }

            guard.end();
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

        let op = Box::new(EditGlyph::new(ch, old_data, new_data));
        self.push_undo_action(op)
    }

    /// Check if clipboard contains BitFont data
    pub fn can_paste(&self) -> bool {
        crate::bitfont::has_bitfont_data()
    }
}
