//! Context-sensitive charset operations for BitFont editor
//!
//! These operations behave differently based on focused_panel:
//! - **EditGrid**: Operate on edit_selection within current glyph (or entire glyph if no selection)
//! - **CharSet**: Operate on all charset_selection glyphs entirely
//!
//! Operations:
//! - Fill (set all pixels to on)
//! - Erase/Clear (set all pixels to off)
//! - Inverse (toggle all pixels)

use crate::bitfont::BitFontUndoOp;
use crate::Result;

use super::{BitFontEditState, BitFontFocusedPanel};

impl BitFontEditState {
    /// Fill pixels with "on" state
    ///
    /// **CharSet focus**: Fills all pixels to "on" for every glyph in the charset selection.
    /// Uses atomic undo so all changes can be undone at once.
    ///
    /// **EditGrid focus**: Fills only the edit_selection rectangle (or entire glyph if none).
    /// Only affects the current `selected_char`.
    pub fn fill_selection(&mut self) -> Result<()> {
        let target_chars = self.get_target_chars();

        if self.focused_panel == BitFontFocusedPanel::CharSet {
            // In CharSet mode, fill entire glyphs for all target chars
            let mut guard = self.begin_atomic_undo("Fill characters");
            for ch in target_chars {
                let old_data = self.get_glyph_pixels(ch).clone();
                let op = BitFontUndoOp::FillSelection {
                    ch,
                    old_data,
                    x1: 0,
                    y1: 0,
                    x2: self.font_width - 1,
                    y2: self.font_height - 1,
                    value: true,
                };
                self.push_undo_action(op)?;
            }
            self.end_atomic_undo(guard.base_count(), guard.description().to_string(), guard.operation_type());
            guard.mark_ended();
            Ok(())
        } else {
            // In EditGrid mode, fill pixel selection for current char
            let sel = self.get_edit_selection_or_all();
            let ch = self.selected_char;
            let old_data = self.get_glyph_pixels(ch).clone();
            let (x1, y1) = (sel.anchor.x, sel.anchor.y);
            let (x2, y2) = (sel.lead.x, sel.lead.y);
            let op = BitFontUndoOp::FillSelection {
                ch,
                old_data,
                x1,
                y1,
                x2,
                y2,
                value: true,
            };
            self.push_undo_action(op)
        }
    }

    /// Erase (clear) pixels to "off" state
    ///
    /// **CharSet focus**: Clears all pixels to "off" for every glyph in the charset selection.
    /// Uses atomic undo so all changes can be undone at once.
    ///
    /// **EditGrid focus**: Clears only the edit_selection rectangle (or entire glyph if none).
    /// Only affects the current `selected_char`.
    pub fn erase_selection(&mut self) -> Result<()> {
        let target_chars = self.get_target_chars();

        if self.focused_panel == BitFontFocusedPanel::CharSet {
            // In CharSet mode, erase entire glyphs for all target chars
            let mut guard = self.begin_atomic_undo("Erase characters");
            for ch in target_chars {
                let old_data = self.get_glyph_pixels(ch).clone();
                let op = BitFontUndoOp::FillSelection {
                    ch,
                    old_data,
                    x1: 0,
                    y1: 0,
                    x2: self.font_width - 1,
                    y2: self.font_height - 1,
                    value: false,
                };
                self.push_undo_action(op)?;
            }
            self.end_atomic_undo(guard.base_count(), guard.description().to_string(), guard.operation_type());
            guard.mark_ended();
            Ok(())
        } else {
            // In EditGrid mode, erase pixel selection for current char
            let sel = self.get_edit_selection_or_all();
            let ch = self.selected_char;
            let old_data = self.get_glyph_pixels(ch).clone();
            let (x1, y1) = (sel.anchor.x, sel.anchor.y);
            let (x2, y2) = (sel.lead.x, sel.lead.y);
            let op = BitFontUndoOp::FillSelection {
                ch,
                old_data,
                x1,
                y1,
                x2,
                y2,
                value: false,
            };
            self.push_undo_action(op)
        }
    }

    /// Inverse (toggle) all pixels in selection
    ///
    /// **CharSet focus**: Inverts all pixels for every glyph in the charset selection.
    /// Uses atomic undo so all changes can be undone at once.
    ///
    /// **EditGrid focus**: Inverts only the edit_selection rectangle (or entire glyph if none).
    /// Only affects the current `selected_char`.
    pub fn inverse_edit_selection(&mut self) -> Result<()> {
        let target_chars = self.get_target_chars();

        if self.focused_panel == BitFontFocusedPanel::CharSet {
            // In CharSet mode, inverse entire glyphs for all target chars
            let mut guard = self.begin_atomic_undo("Inverse characters");
            for ch in target_chars {
                let op = BitFontUndoOp::InverseGlyph { ch };
                self.push_undo_action(op)?;
            }
            self.end_atomic_undo(guard.base_count(), guard.description().to_string(), guard.operation_type());
            guard.mark_ended();
            Ok(())
        } else {
            // In EditGrid mode, inverse pixel selection for current char
            let sel = self.get_edit_selection_or_all();
            let ch = self.selected_char;
            let old_data = self.get_glyph_pixels(ch).clone();
            let (x1, y1) = (sel.anchor.x, sel.anchor.y);
            let (x2, y2) = (sel.lead.x, sel.lead.y);
            let op = BitFontUndoOp::InverseSelection { ch, old_data, x1, y1, x2, y2 };
            self.push_undo_action(op)
        }
    }
}
