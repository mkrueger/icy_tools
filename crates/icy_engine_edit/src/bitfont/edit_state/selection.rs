//! Selection handling for BitFont editor
//!
//! Handles two types of selection:
//! - **Edit Selection**: Pixel-level rectangle selection within a single glyph
//! - **Charset Selection**: Character-level selection in the 16×16 charset grid
//!
//! When one selection is set, the other is automatically cleared.
//! Selection changes are undoable and include cursor positions.
//!
//! Charset selection has two modes:
//! - **Linear mode** (default): Characters selected in reading order (left-to-right, top-to-bottom)
//! - **Rectangle mode** (Alt+drag): Characters selected in a rectangular region

use icy_engine::{Position, Selection, Shape};

use crate::bitfont::BitFontUndoOp;

use super::{BitFontEditState, BitFontFocusedPanel};

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Edit Selection (pixel-level within glyph)
    // ═══════════════════════════════════════════════════════════════════════

    /// Get edit selection (rectangular pixel selection)
    pub fn edit_selection(&self) -> Option<Selection> {
        self.edit_selection
    }

    /// Get edit selection, or entire glyph if no selection exists
    pub fn get_edit_selection_or_all(&self) -> Selection {
        self.edit_selection.unwrap_or_else(|| {
            let mut sel = Selection::new((0, 0));
            sel.lead = Position::new(self.font_width - 1, self.font_height - 1);
            sel.shape = Shape::Rectangle;
            sel
        })
    }

    /// Get selection rectangle as (x1, y1, x2, y2) for backwards compatibility
    pub fn selection(&self) -> Option<(i32, i32, i32, i32)> {
        self.edit_selection.map(|s| (s.anchor.x, s.anchor.y, s.lead.x, s.lead.y))
    }

    /// Set selection rectangle (backwards compatibility)
    pub fn set_selection(&mut self, sel: Option<(i32, i32, i32, i32)>) {
        self.edit_selection = sel.map(|s| {
            let mut selection = Selection::new((s.0, s.1));
            selection.lead = Position::new(s.2, s.3);
            selection.shape = Shape::Rectangle;
            selection
        });
    }

    /// Start edit selection from current cursor position (with undo)
    ///
    /// This clears any charset selection and pushes an undo operation.
    pub fn start_edit_selection(&mut self) {
        let old_edit = self.edit_selection;
        let old_charset = self.charset_selection;
        let old_cursor = self.cursor_pos;
        let old_charset_cursor = self.charset_cursor;

        let (x, y) = self.cursor_pos;
        let mut sel = Selection::new((x, y));
        sel.shape = Shape::Rectangle;

        // Set new selection and clear charset selection
        self.edit_selection = Some(sel);
        self.charset_selection = None;

        // Push undo operation
        let op = BitFontUndoOp::SelectionChange {
            old_edit_selection: old_edit,
            new_edit_selection: self.edit_selection,
            old_charset_selection: old_charset,
            new_charset_selection: None,
            old_cursor_pos: old_cursor,
            new_cursor_pos: self.cursor_pos,
            old_charset_cursor,
            new_charset_cursor: self.charset_cursor,
        };
        let _ = self.push_plain_undo(op);
    }

    /// Extend edit selection to current cursor position (with undo)
    pub fn extend_edit_selection(&mut self) {
        if let Some(ref mut sel) = self.edit_selection {
            let old_edit = Some(*sel);

            let (x, y) = self.cursor_pos;
            sel.lead = Position::new(x, y);

            // Only push undo if selection actually changed
            if old_edit.map(|s| s.lead) != self.edit_selection.map(|s| s.lead) {
                let op = BitFontUndoOp::SelectionChange {
                    old_edit_selection: old_edit,
                    new_edit_selection: self.edit_selection,
                    old_charset_selection: self.charset_selection,
                    new_charset_selection: self.charset_selection,
                    old_cursor_pos: self.cursor_pos,
                    new_cursor_pos: self.cursor_pos,
                    old_charset_cursor: self.charset_cursor,
                    new_charset_cursor: self.charset_cursor,
                };
                let _ = self.push_plain_undo(op);
            }
        }
    }

    /// Move cursor and extend selection in one undo operation
    ///
    /// This captures the old state before moving, then pushes a single undo.
    /// If no selection exists, one is started at the current cursor position.
    pub fn move_cursor_and_extend_selection(&mut self, dx: i32, dy: i32) {
        // Capture old state before any changes
        let old_edit = self.edit_selection;
        let old_cursor = self.cursor_pos;

        // If no selection exists, start one at current cursor position
        let anchor = if let Some(sel) = self.edit_selection {
            sel.anchor
        } else {
            let (x, y) = self.cursor_pos;
            Position::new(x, y)
        };

        // Move cursor with wrapping
        let (x, y) = self.cursor_pos;
        let new_x = (x + dx).rem_euclid(self.font_width);
        let new_y = (y + dy).rem_euclid(self.font_height);
        self.cursor_pos = (new_x, new_y);

        // Set selection with anchor and new lead position
        let lead = Position::new(new_x, new_y);
        let mut new_sel = Selection::new(anchor);
        new_sel.lead = lead;
        self.edit_selection = Some(new_sel);

        // Push single undo operation for cursor move + selection extend
        let op = BitFontUndoOp::SelectionChange {
            old_edit_selection: old_edit,
            new_edit_selection: self.edit_selection,
            old_charset_selection: self.charset_selection,
            new_charset_selection: self.charset_selection,
            old_cursor_pos: old_cursor,
            new_cursor_pos: self.cursor_pos,
            old_charset_cursor: self.charset_cursor,
            new_charset_cursor: self.charset_cursor,
        };
        let _ = self.push_plain_undo(op);
    }

    /// Clear edit selection (with undo)
    pub fn clear_edit_selection(&mut self) {
        if self.edit_selection.is_some() {
            let old_edit: Option<Selection> = self.edit_selection;

            self.edit_selection = None;

            let op = BitFontUndoOp::SelectionChange {
                old_edit_selection: old_edit,
                new_edit_selection: None,
                old_charset_selection: self.charset_selection,
                new_charset_selection: self.charset_selection,
                old_cursor_pos: self.cursor_pos,
                new_cursor_pos: self.cursor_pos,
                old_charset_cursor: self.charset_cursor,
                new_charset_cursor: self.charset_cursor,
            };
            let _ = self.push_plain_undo(op);
        }
    }

    /// Clear selection (alias for clear_edit_selection for backwards compatibility)
    pub fn clear_selection(&mut self) {
        self.clear_edit_selection();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Charset Selection (character-level)
    // ═══════════════════════════════════════════════════════════════════════

    /// Get charset selection (anchor, lead, is_rectangle)
    /// - is_rectangle: false = linear selection (default), true = rectangle (Alt+drag)
    pub fn charset_selection(&self) -> Option<(Position, Position, bool)> {
        self.charset_selection
    }

    /// Set charset selection using anchor/lead tuple with rectangle mode
    pub fn set_charset_selection(&mut self, sel: Option<(Position, Position, bool)>) {
        self.charset_selection = sel;
    }

    /// Start charset selection from current charset cursor position (with undo)
    ///
    /// This clears any edit selection and pushes an undo operation.
    /// is_rectangle: false = linear (default), true = rectangle (Alt held)
    pub fn start_charset_selection_with_mode(&mut self, is_rectangle: bool) {
        let old_edit = self.edit_selection;
        let old_charset = self.charset_selection;
        let old_cursor = self.cursor_pos;
        let old_charset_cursor = self.charset_cursor;

        let (x, y) = self.charset_cursor;
        let anchor = Position::new(x, y);
        let new_charset = Some((anchor, anchor, is_rectangle));

        // Set new selection and clear edit selection
        self.charset_selection = new_charset;
        self.edit_selection = None;

        // Push undo operation
        let op = BitFontUndoOp::SelectionChange {
            old_edit_selection: old_edit,
            new_edit_selection: None,
            old_charset_selection: old_charset,
            new_charset_selection: new_charset,
            old_cursor_pos: old_cursor,
            new_cursor_pos: self.cursor_pos,
            old_charset_cursor,
            new_charset_cursor: self.charset_cursor,
        };
        let _ = self.push_plain_undo(op);
    }

    /// Start charset selection (linear mode, for backwards compatibility)
    pub fn start_charset_selection(&mut self) {
        self.start_charset_selection_with_mode(false);
    }

    /// Extend charset selection to current charset cursor position (with undo)
    /// Optionally update rectangle mode (e.g., if Alt key state changed)
    pub fn extend_charset_selection_with_mode(&mut self, is_rectangle: bool) {
        if let Some((anchor, old_lead, _)) = self.charset_selection {
            let old_charset = self.charset_selection;
            let old_charset_cursor = self.charset_cursor;

            let (x, y) = self.charset_cursor;
            let lead = Position::new(x, y);
            let new_charset = Some((anchor, lead, is_rectangle));
            self.charset_selection = new_charset;

            // Only push undo if selection actually changed
            if old_lead != lead {
                let op = BitFontUndoOp::SelectionChange {
                    old_edit_selection: self.edit_selection,
                    new_edit_selection: self.edit_selection,
                    old_charset_selection: old_charset,
                    new_charset_selection: new_charset,
                    old_cursor_pos: self.cursor_pos,
                    new_cursor_pos: self.cursor_pos,
                    old_charset_cursor,
                    new_charset_cursor: self.charset_cursor,
                };
                let _ = self.push_plain_undo(op);
            }
        }
    }

    /// Extend charset selection (preserves current rectangle mode)
    pub fn extend_charset_selection(&mut self) {
        if let Some((_, _, is_rect)) = self.charset_selection {
            self.extend_charset_selection_with_mode(is_rect);
        }
    }

    /// Move charset cursor and extend selection in one undo operation
    ///
    /// This captures the old state before moving, then pushes a single undo.
    /// If no selection exists, one is started at the current cursor position.
    pub fn move_charset_cursor_and_extend_selection(&mut self, dx: i32, dy: i32, is_rectangle: bool) {
        // Capture old state before any changes
        let old_charset = self.charset_selection;
        let old_charset_cursor = self.charset_cursor;

        // If no selection exists, start one at current cursor position
        let anchor = if let Some((anchor, _, _)) = self.charset_selection {
            anchor
        } else {
            let (x, y) = self.charset_cursor;
            Position::new(x, y)
        };

        // Move charset cursor with wrapping
        let (x, y) = self.charset_cursor;
        let new_x = (x + dx).rem_euclid(16);
        let new_y = (y + dy).rem_euclid(16);
        self.charset_cursor = (new_x, new_y);

        // Set selection with anchor and new lead position
        let lead = Position::new(new_x, new_y);
        self.charset_selection = Some((anchor, lead, is_rectangle));

        // Push undo operation WITHOUT executing redo (state already changed)
        let op = BitFontUndoOp::SelectionChange {
            old_edit_selection: self.edit_selection,
            new_edit_selection: self.edit_selection,
            old_charset_selection: old_charset,
            new_charset_selection: self.charset_selection,
            old_cursor_pos: self.cursor_pos,
            new_cursor_pos: self.cursor_pos,
            old_charset_cursor,
            new_charset_cursor: self.charset_cursor,
        };
        let _result = self.push_plain_undo(op);
    }

    /// Clear charset selection (with undo)
    pub fn clear_charset_selection(&mut self) {
        if self.charset_selection.is_some() {
            let old_charset: Option<(Position, Position, bool)> = self.charset_selection;

            self.charset_selection = None;

            let op = BitFontUndoOp::SelectionChange {
                old_edit_selection: self.edit_selection,
                new_edit_selection: self.edit_selection,
                old_charset_selection: old_charset,
                new_charset_selection: None,
                old_cursor_pos: self.cursor_pos,
                new_cursor_pos: self.cursor_pos,
                old_charset_cursor: self.charset_cursor,
                new_charset_cursor: self.charset_cursor,
            };
            let _ = self.push_plain_undo(op);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Target Characters (context-sensitive)
    // ═══════════════════════════════════════════════════════════════════════

    /// Get all characters that should be affected by operations
    ///
    /// The result depends on the focused panel:
    ///
    /// **CharSet focus with selection**:
    /// - Linear mode (default): all characters from anchor to lead in reading order
    /// - Rectangle mode (Alt): all characters in the rectangular region
    ///
    /// **CharSet focus without selection**: character at charset cursor
    ///
    /// **EditGrid focus**: just the currently selected character
    pub fn get_target_chars(&self) -> Vec<char> {
        match self.focused_panel {
            BitFontFocusedPanel::CharSet => {
                if let Some((anchor, lead, is_rectangle)) = self.charset_selection {
                    if is_rectangle {
                        // Rectangle mode: get all characters in the bounding rectangle
                        let min_x = anchor.x.min(lead.x).max(0);
                        let max_x = anchor.x.max(lead.x).min(15);
                        let min_y = anchor.y.min(lead.y).max(0);
                        let max_y = anchor.y.max(lead.y).min(15);

                        let mut chars = Vec::new();
                        for y in min_y..=max_y {
                            for x in min_x..=max_x {
                                let code = (y * 16 + x) as u32;
                                if let Some(ch) = char::from_u32(code) {
                                    chars.push(ch);
                                }
                            }
                        }
                        chars
                    } else {
                        // Linear mode: get all characters from anchor to lead
                        let anchor_code = (anchor.y * 16 + anchor.x) as u32;
                        let lead_code = (lead.y * 16 + lead.x) as u32;
                        let (start, end) = if anchor_code <= lead_code {
                            (anchor_code, lead_code)
                        } else {
                            (lead_code, anchor_code)
                        };

                        let mut chars = Vec::new();
                        for code in start..=end {
                            if let Some(ch) = char::from_u32(code) {
                                chars.push(ch);
                            }
                        }
                        chars
                    }
                } else {
                    // Just the character at charset cursor
                    vec![self.char_at_charset_cursor()]
                }
            }
            BitFontFocusedPanel::EditGrid => {
                // Just the currently selected character
                vec![self.selected_char]
            }
        }
    }
}
