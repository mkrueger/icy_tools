//! Terminal input handling for text editors
//!
//! This module provides Moebius-compatible text input functionality with proper undo support.
//! All operations are atomic and integrate with the undo system.
//!
//! # Features
//! - Character input with insert/overwrite mode
//! - Backspace and delete handling
//! - Tab navigation (8-column stops)
//! - New line handling (insert mode: insert row, then move)
//! - Mirror mode support (same char at mirrored position)
//!
//! # Usage
//! ```ignore
//! // Type a character
//! edit_state.type_key('A')?;
//!
//! // Toggle insert mode
//! edit_state.toggle_insert_mode();
//!
//! // Backspace
//! edit_state.backspace()?;
//! ```

use i18n_embed_fl::fl;

use crate::{AttributedChar, Position, Result, TextPane};

use super::{EditState, EditorUndoOp};

impl EditState {
    /// Type a single character at the caret position.
    ///
    /// In insert mode, characters to the right are shifted.
    /// In overwrite mode, the character is simply replaced.
    /// Caret moves right after typing (unless in a special "overwrite_mode" where it stays).
    ///
    /// Mirror mode: if enabled, the same character is placed at the mirrored x position.
    pub fn type_key(&mut self, char_code: char) -> Result<()> {
        let pos = self.get_caret().position();
        let layer_idx = self.get_current_layer()?;
        let insert_mode = self.get_caret().insert_mode;
        let caret_attr = self.get_caret().attribute.clone();
        let mirror_mode = self.mirror_mode;

        // Get layer dimensions and characters we need
        let (layer_width, old_char, chars_to_shift) = {
            let Some(layer) = self.get_cur_layer() else {
                return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
            };

            // Check bounds
            if pos.x < 0 || pos.x >= layer.width() || pos.y < 0 || pos.y >= layer.height() {
                return Ok(());
            }

            let mut chars_to_shift = Vec::new();
            if insert_mode {
                // Collect characters to shift
                for x in pos.x..layer.width() {
                    chars_to_shift.push(layer.char_at(Position::new(x, pos.y)));
                }
            }

            (layer.width(), layer.char_at(pos), chars_to_shift)
        };

        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-type_char"));

        // Handle insert mode: shift characters to the right
        if insert_mode && chars_to_shift.len() > 1 {
            // Shift from end to current position (right to left)
            for i in (1..chars_to_shift.len()).rev() {
                let cur_pos = Position::new(pos.x + i as i32, pos.y);
                let old = chars_to_shift[i];
                let new = chars_to_shift[i - 1];

                self.push_undo_action(EditorUndoOp::SetChar {
                    pos: cur_pos,
                    layer: layer_idx,
                    old,
                    new,
                })?;
            }
        }

        // Build the attributed character with caret attributes
        let new_char = AttributedChar::new(char_code, caret_attr.clone());

        // Set the character
        self.push_undo_action(EditorUndoOp::SetChar {
            pos,
            layer: layer_idx,
            old: old_char,
            new: new_char,
        })?;

        // Handle mirror mode: set same character at mirrored position
        if mirror_mode {
            let mirror_x = layer_width - pos.x - 1;
            if mirror_x != pos.x && mirror_x >= 0 {
                let mirror_pos = Position::new(mirror_x, pos.y);
                // Need to get mirror_old from layer
                let mirror_old = self.get_cur_layer().map(|l| l.char_at(mirror_pos)).unwrap_or_default();
                // For now, use the same character (no flip_code_x)
                // TODO: Implement character mirroring table for box drawing chars
                self.push_undo_action(EditorUndoOp::SetChar {
                    pos: mirror_pos,
                    layer: layer_idx,
                    old: mirror_old,
                    new: new_char,
                })?;
            }
        }

        // Move caret right
        let new_x = (pos.x + 1).min(layer_width - 1);
        self.get_caret_mut().x = new_x;

        Ok(())
    }

    /// Handle backspace key.
    ///
    /// Moves caret left and:
    /// - In insert mode: shifts characters left (deletes the character)
    /// - In overwrite mode: just clears the character at the new position
    pub fn backspace(&mut self) -> Result<()> {
        let pos = self.get_caret().position();

        // Can't backspace at column 0
        if pos.x <= 0 {
            return Ok(());
        }

        let layer_idx = self.get_current_layer()?;
        let insert_mode = self.get_caret().insert_mode;
        let caret_attr = self.get_caret().attribute.clone();

        // Collect data we need from layer
        let (layer_width, chars_to_shift, delete_pos_char) = {
            let Some(layer) = self.get_cur_layer() else {
                return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
            };

            let delete_pos = Position::new(pos.x - 1, pos.y);
            let delete_pos_char = layer.char_at(delete_pos);

            let mut chars_to_shift = Vec::new();
            if insert_mode {
                for x in (pos.x - 1)..layer.width() {
                    chars_to_shift.push(layer.char_at(Position::new(x, pos.y)));
                }
            }

            (layer.width(), chars_to_shift, delete_pos_char)
        };

        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-backspace"));

        // Move caret left first
        let new_x = pos.x - 1;
        self.get_caret_mut().x = new_x;

        if insert_mode && !chars_to_shift.is_empty() {
            // Insert mode: shift all characters left
            for i in 0..chars_to_shift.len() - 1 {
                let cur_pos = Position::new(new_x + i as i32, pos.y);
                let old = chars_to_shift[i];
                let new = chars_to_shift[i + 1];

                self.push_undo_action(EditorUndoOp::SetChar {
                    pos: cur_pos,
                    layer: layer_idx,
                    old,
                    new,
                })?;
            }

            // Clear the last character
            let last_pos = Position::new(layer_width - 1, pos.y);
            let last_char = chars_to_shift[chars_to_shift.len() - 1];
            let empty_char = AttributedChar::new(' ', caret_attr);
            self.push_undo_action(EditorUndoOp::SetChar {
                pos: last_pos,
                layer: layer_idx,
                old: last_char,
                new: empty_char,
            })?;
        } else {
            // Overwrite mode: just clear the character
            let empty_char = AttributedChar::new(' ', caret_attr);
            self.push_undo_action(EditorUndoOp::SetChar {
                pos: Position::new(new_x, pos.y),
                layer: layer_idx,
                old: delete_pos_char,
                new: empty_char,
            })?;
        }

        Ok(())
    }

    /// Handle delete key.
    ///
    /// Deletes the character at the current position and shifts remaining characters left.
    pub fn delete_key(&mut self) -> Result<()> {
        let pos = self.get_caret().position();
        let layer_idx = self.get_current_layer()?;
        let caret_attr = self.get_caret().attribute.clone();

        // Collect data from layer
        let (layer_width, chars_to_shift) = {
            let Some(layer) = self.get_cur_layer() else {
                return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
            };

            // Check bounds
            if pos.x < 0 || pos.x >= layer.width() || pos.y < 0 || pos.y >= layer.height() {
                return Ok(());
            }

            let mut chars_to_shift = Vec::new();
            for x in pos.x..layer.width() {
                chars_to_shift.push(layer.char_at(Position::new(x, pos.y)));
            }

            (layer.width(), chars_to_shift)
        };

        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete"));

        // Shift all characters left from current position
        for i in 0..chars_to_shift.len() - 1 {
            let cur_pos = Position::new(pos.x + i as i32, pos.y);
            let old = chars_to_shift[i];
            let new = chars_to_shift[i + 1];

            self.push_undo_action(EditorUndoOp::SetChar {
                pos: cur_pos,
                layer: layer_idx,
                old,
                new,
            })?;
        }

        // Clear the last character
        let last_pos = Position::new(layer_width - 1, pos.y);
        let last_char = chars_to_shift[chars_to_shift.len() - 1];
        let empty_char = AttributedChar::new(' ', caret_attr);
        self.push_undo_action(EditorUndoOp::SetChar {
            pos: last_pos,
            layer: layer_idx,
            old: last_char,
            new: empty_char,
        })?;

        Ok(())
    }

    /// Handle new line (Enter key).
    ///
    /// Moves caret to the beginning of the next line.
    /// In insert mode, inserts a new row first.
    pub fn new_line(&mut self) -> Result<()> {
        let pos = self.get_caret().position();
        let insert_mode = self.get_caret().insert_mode;

        let layer_height = {
            let Some(layer) = self.get_cur_layer() else {
                return Err(crate::EngineError::Generic("Current layer is invalid".to_string()));
            };
            layer.height()
        };

        // In insert mode, insert a row (if not at last row)
        if insert_mode && pos.y < layer_height - 1 {
            // First move to next row
            let new_y = pos.y + 1;
            self.get_caret_mut().y = new_y;
            self.get_caret_mut().x = 0;
            // Insert row at new position
            self.insert_row()?;
        } else {
            // Just move to beginning of next line
            let new_y = (pos.y + 1).min(layer_height - 1);
            self.get_caret_mut().y = new_y;
            self.get_caret_mut().x = 0;
        }

        Ok(())
    }

    /// Handle tab key.
    ///
    /// Moves caret to the next 8-column tab stop.
    pub fn handle_tab(&mut self) {
        let pos = self.get_caret().position();
        let layer_width = self.get_cur_layer().map(|l| l.width()).unwrap_or(80);

        // Move to next tab stop (8-column increments)
        let new_x = ((pos.x / 8) + 1) * 8;
        let new_x = new_x.min(layer_width - 1);
        self.get_caret_mut().x = new_x;
    }

    /// Handle reverse tab (Shift+Tab).
    ///
    /// Moves caret to the previous 8-column tab stop.
    pub fn handle_reverse_tab(&mut self) {
        let pos = self.get_caret().position();

        // Move to previous tab stop (8-column increments)
        let new_x = if pos.x % 8 == 0 { (pos.x - 8).max(0) } else { (pos.x / 8) * 8 };
        self.get_caret_mut().x = new_x;
    }

    /// Toggle insert/overwrite mode.
    pub fn toggle_insert_mode(&mut self) {
        let current = self.get_caret().insert_mode;
        self.get_caret_mut().insert_mode = !current;
    }
}
