//! Undo operations for BitFont editing
//!
//! Contains the serializable enum-based undo operation type.

use icy_engine::{Position, Selection};
use serde::{Deserialize, Serialize};

use crate::bitfont::BitFontEditState;
use crate::Result;

/// Type of operation for grouping related undos
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitFontOperationType {
    /// Unknown/default operation
    Unknown,
    /// Pixel editing (drawing)
    EditPixels,
    /// Glyph transformation (move, flip, etc.)
    Transform,
    /// Font resize
    Resize,
}

/// Serializable undo operation enum for BitFont editing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BitFontUndoOp {
    /// Atomic group of operations
    Atomic {
        description: String,
        operations: Vec<BitFontUndoOp>,
        operation_type: BitFontOperationType,
    },

    /// Edit glyph pixels
    EditGlyph {
        ch: char,
        old_data: Vec<Vec<bool>>,
        new_data: Vec<Vec<bool>>,
    },

    /// Clear glyph (set all pixels to off)
    ClearGlyph { ch: char, old_data: Vec<Vec<bool>> },

    /// Delete a column at specified X position
    DeleteColumn {
        x_pos: usize,
        old_width: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Delete a row at specified Y position
    DeleteLine {
        y_pos: usize,
        old_height: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Duplicate a row at specified Y position
    DuplicateLine {
        y_pos: usize,
        old_height: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Fill a rectangular region
    FillSelection {
        ch: char,
        old_data: Vec<Vec<bool>>,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        value: bool,
    },

    /// Flip glyph horizontally or vertically
    FlipGlyph {
        ch: char,
        horizontal: bool,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },

    /// Insert a column at specified X position
    InsertColumn {
        x_pos: usize,
        old_width: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Insert a row at specified Y position
    InsertLine {
        y_pos: usize,
        old_height: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Inverse glyph (toggle all pixels)
    InverseGlyph { ch: char },

    /// Inverse a rectangular region
    InverseSelection {
        ch: char,
        old_data: Vec<Vec<bool>>,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },

    /// Move glyph pixels by offset
    MoveGlyph { ch: char, dx: i32, dy: i32, old_data: Vec<Vec<bool>> },

    /// Resize all glyphs in the font
    ResizeFont {
        old_width: i32,
        old_height: i32,
        new_width: i32,
        new_height: i32,
        old_glyph_data: Vec<Vec<Vec<bool>>>,
    },

    /// Selection change (edit and charset selection, cursor positions)
    SelectionChange {
        old_edit_selection: Option<Selection>,
        new_edit_selection: Option<Selection>,
        old_charset_selection: Option<(Position, Position, bool)>,
        new_charset_selection: Option<(Position, Position, bool)>,
        old_cursor_pos: (i32, i32),
        new_cursor_pos: (i32, i32),
        old_charset_cursor: (i32, i32),
        new_charset_cursor: (i32, i32),
    },

    /// Swap two characters' glyph data
    SwapChars {
        char1: char,
        char2: char,
        data1: Vec<Vec<bool>>,
        data2: Vec<Vec<bool>>,
    },
}

impl BitFontUndoOp {
    /// Get a description of this operation for display
    pub fn get_description(&self) -> String {
        match self {
            BitFontUndoOp::Atomic { description, .. } => description.clone(),
            BitFontUndoOp::EditGlyph { .. } => "Edit glyph".to_string(),
            BitFontUndoOp::ClearGlyph { .. } => "Clear glyph".to_string(),
            BitFontUndoOp::DeleteColumn { .. } => "Delete column".to_string(),
            BitFontUndoOp::DeleteLine { .. } => "Delete line".to_string(),
            BitFontUndoOp::DuplicateLine { .. } => "Duplicate line".to_string(),
            BitFontUndoOp::FillSelection { value, .. } => {
                if *value {
                    "Fill selection".to_string()
                } else {
                    "Erase selection".to_string()
                }
            }
            BitFontUndoOp::FlipGlyph { horizontal, .. } => {
                if *horizontal {
                    "Flip horizontal".to_string()
                } else {
                    "Flip vertical".to_string()
                }
            }
            BitFontUndoOp::InsertColumn { .. } => "Insert column".to_string(),
            BitFontUndoOp::InsertLine { .. } => "Insert line".to_string(),
            BitFontUndoOp::InverseGlyph { .. } => "Inverse glyph".to_string(),
            BitFontUndoOp::InverseSelection { .. } => "Inverse selection".to_string(),
            BitFontUndoOp::MoveGlyph { dx, dy, .. } => match (*dx, *dy) {
                (0, -1) => "Move up".to_string(),
                (0, 1) => "Move down".to_string(),
                (-1, 0) => "Move left".to_string(),
                (1, 0) => "Move right".to_string(),
                _ => "Move glyph".to_string(),
            },
            BitFontUndoOp::ResizeFont { .. } => "Resize font".to_string(),
            BitFontUndoOp::SelectionChange { .. } => "Selection change".to_string(),
            BitFontUndoOp::SwapChars { .. } => "Swap characters".to_string(),
        }
    }

    /// Get the operation type for grouping
    pub fn get_operation_type(&self) -> BitFontOperationType {
        match self {
            BitFontUndoOp::Atomic { operation_type, .. } => *operation_type,
            BitFontUndoOp::EditGlyph { .. } => BitFontOperationType::EditPixels,
            BitFontUndoOp::ClearGlyph { .. } => BitFontOperationType::EditPixels,
            BitFontUndoOp::DeleteColumn { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::DeleteLine { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::DuplicateLine { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::FillSelection { .. } => BitFontOperationType::EditPixels,
            BitFontUndoOp::FlipGlyph { .. } => BitFontOperationType::Transform,
            BitFontUndoOp::InsertColumn { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::InsertLine { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::InverseGlyph { .. } => BitFontOperationType::Transform,
            BitFontUndoOp::InverseSelection { .. } => BitFontOperationType::Transform,
            BitFontUndoOp::MoveGlyph { .. } => BitFontOperationType::Transform,
            BitFontUndoOp::ResizeFont { .. } => BitFontOperationType::Resize,
            BitFontUndoOp::SelectionChange { .. } => BitFontOperationType::Unknown,
            BitFontUndoOp::SwapChars { .. } => BitFontOperationType::Transform,
        }
    }

    /// Whether this operation changes data (affects dirty flag)
    pub fn changes_data(&self) -> bool {
        match self {
            BitFontUndoOp::Atomic { operations, .. } => operations.iter().any(|op| op.changes_data()),
            BitFontUndoOp::SelectionChange { .. } => false,
            _ => true,
        }
    }

    /// Undo this operation
    pub fn undo(&self, state: &mut BitFontEditState) -> Result<()> {
        match self {
            BitFontUndoOp::Atomic { operations, .. } => {
                for op in operations.iter().rev() {
                    op.undo(state)?;
                }
                Ok(())
            }

            BitFontUndoOp::EditGlyph { ch, old_data, .. } => {
                state.set_glyph_pixels_internal(*ch, old_data.clone());
                Ok(())
            }

            BitFontUndoOp::ClearGlyph { ch, old_data } => {
                state.set_glyph_pixels_internal(*ch, old_data.clone());
                Ok(())
            }

            BitFontUndoOp::DeleteColumn { old_width, old_glyph_data, .. } => {
                let (_, height) = state.font_size();
                state.set_font_dimensions_internal(*old_width, height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::DeleteLine {
                old_height, old_glyph_data, ..
            } => {
                let (width, _) = state.font_size();
                state.set_font_dimensions_internal(width, *old_height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::DuplicateLine {
                old_height, old_glyph_data, ..
            } => {
                let (width, _) = state.font_size();
                state.set_font_dimensions_internal(width, *old_height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::FillSelection { ch, old_data, .. } => {
                state.set_glyph_pixels_internal(*ch, old_data.clone());
                Ok(())
            }

            BitFontUndoOp::FlipGlyph {
                ch,
                horizontal,
                x1,
                y1,
                x2,
                y2,
            } => {
                // Flip is self-reversing
                self.do_flip(state, *ch, *horizontal, *x1, *y1, *x2, *y2)
            }

            BitFontUndoOp::InsertColumn { old_width, old_glyph_data, .. } => {
                let (_, height) = state.font_size();
                state.set_font_dimensions_internal(*old_width, height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::InsertLine {
                old_height, old_glyph_data, ..
            } => {
                let (width, _) = state.font_size();
                state.set_font_dimensions_internal(width, *old_height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::InverseGlyph { ch } => {
                // Inverse is self-reversing
                self.do_inverse_glyph(state, *ch)
            }

            BitFontUndoOp::InverseSelection { ch, old_data, .. } => {
                state.set_glyph_pixels_internal(*ch, old_data.clone());
                Ok(())
            }

            BitFontUndoOp::MoveGlyph { ch, old_data, .. } => {
                state.set_glyph_pixels_internal(*ch, old_data.clone());
                Ok(())
            }

            BitFontUndoOp::ResizeFont {
                old_width,
                old_height,
                old_glyph_data,
                ..
            } => {
                state.set_font_dimensions_internal(*old_width, *old_height);
                for (i, glyph_data) in old_glyph_data.iter().enumerate() {
                    if let Some(ch) = char::from_u32(i as u32) {
                        state.set_glyph_pixels_internal(ch, glyph_data.clone());
                    }
                }
                Ok(())
            }

            BitFontUndoOp::SelectionChange {
                old_edit_selection,
                old_charset_selection,
                old_cursor_pos,
                old_charset_cursor,
                ..
            } => {
                state.set_edit_selection_internal(*old_edit_selection);
                state.set_charset_selection_internal(*old_charset_selection);
                state.set_cursor_pos_internal(*old_cursor_pos);
                state.set_charset_cursor_internal(*old_charset_cursor);
                Ok(())
            }

            BitFontUndoOp::SwapChars { char1, char2, data1, data2 } => {
                // Restore original data
                state.set_glyph_pixels_internal(*char1, data1.clone());
                state.set_glyph_pixels_internal(*char2, data2.clone());
                Ok(())
            }
        }
    }

    /// Redo this operation
    pub fn redo(&self, state: &mut BitFontEditState) -> Result<()> {
        match self {
            BitFontUndoOp::Atomic { operations, .. } => {
                for op in operations.iter() {
                    op.redo(state)?;
                }
                Ok(())
            }

            BitFontUndoOp::EditGlyph { ch, new_data, .. } => {
                state.set_glyph_pixels_internal(*ch, new_data.clone());
                Ok(())
            }

            BitFontUndoOp::ClearGlyph { ch, .. } => {
                let (width, height) = state.font_size();
                let cleared = vec![vec![false; width as usize]; height as usize];
                state.set_glyph_pixels_internal(*ch, cleared);
                Ok(())
            }

            BitFontUndoOp::DeleteColumn { x_pos, .. } => {
                state.delete_column_internal(*x_pos);
                Ok(())
            }

            BitFontUndoOp::DeleteLine { y_pos, .. } => {
                state.delete_line_internal(*y_pos);
                Ok(())
            }

            BitFontUndoOp::DuplicateLine { y_pos, .. } => {
                state.duplicate_line_internal(*y_pos);
                Ok(())
            }

            BitFontUndoOp::FillSelection { ch, x1, y1, x2, y2, value, .. } => {
                state.fill_region_internal(*ch, *x1, *y1, *x2, *y2, *value);
                Ok(())
            }

            BitFontUndoOp::FlipGlyph {
                ch,
                horizontal,
                x1,
                y1,
                x2,
                y2,
            } => self.do_flip(state, *ch, *horizontal, *x1, *y1, *x2, *y2),

            BitFontUndoOp::InsertColumn { x_pos, .. } => {
                state.insert_column_internal(*x_pos);
                Ok(())
            }

            BitFontUndoOp::InsertLine { y_pos, .. } => {
                state.insert_line_internal(*y_pos);
                Ok(())
            }

            BitFontUndoOp::InverseGlyph { ch } => self.do_inverse_glyph(state, *ch),

            BitFontUndoOp::InverseSelection { ch, x1, y1, x2, y2, .. } => {
                state.inverse_region_internal(*ch, *x1, *y1, *x2, *y2);
                Ok(())
            }

            BitFontUndoOp::MoveGlyph { ch, dx, dy, old_data } => {
                let (width, height) = state.font_size();
                let mut new_data = vec![vec![false; width as usize]; height as usize];

                for y in 0..height as usize {
                    for x in 0..width as usize {
                        let src_x = (x as i32 - dx).rem_euclid(width) as usize;
                        let src_y = (y as i32 - dy).rem_euclid(height) as usize;

                        if let Some(row) = old_data.get(src_y) {
                            if let Some(&pixel) = row.get(src_x) {
                                new_data[y][x] = pixel;
                            }
                        }
                    }
                }

                state.set_glyph_pixels_internal(*ch, new_data);
                Ok(())
            }

            BitFontUndoOp::ResizeFont { new_width, new_height, .. } => {
                state.resize_glyphs_internal(*new_width, *new_height);
                Ok(())
            }

            BitFontUndoOp::SelectionChange {
                new_edit_selection,
                new_charset_selection,
                new_cursor_pos,
                new_charset_cursor,
                ..
            } => {
                state.set_edit_selection_internal(*new_edit_selection);
                state.set_charset_selection_internal(*new_charset_selection);
                state.set_cursor_pos_internal(*new_cursor_pos);
                state.set_charset_cursor_internal(*new_charset_cursor);
                Ok(())
            }

            BitFontUndoOp::SwapChars { char1, char2, data1, data2 } => {
                // Swap: put data1 in char2 and data2 in char1
                state.set_glyph_pixels_internal(*char1, data2.clone());
                state.set_glyph_pixels_internal(*char2, data1.clone());
                Ok(())
            }
        }
    }

    // Helper for flip operation (self-reversing)
    fn do_flip(&self, state: &mut BitFontEditState, ch: char, horizontal: bool, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()> {
        let mut data = state.get_glyph_pixels(ch).clone();

        let min_x = x1.min(x2) as usize;
        let max_x = x1.max(x2) as usize;
        let min_y = y1.min(y2) as usize;
        let max_y = y1.max(y2) as usize;

        if horizontal {
            for y in min_y..=max_y {
                if y < data.len() {
                    let row = &mut data[y];
                    let mut left = min_x;
                    let mut right = max_x;
                    while left < right {
                        if left < row.len() && right < row.len() {
                            row.swap(left, right);
                        }
                        left += 1;
                        right -= 1;
                    }
                }
            }
        } else {
            let mut top = min_y;
            let mut bottom = max_y;
            while top < bottom {
                if top < data.len() && bottom < data.len() {
                    for x in min_x..=max_x {
                        if x < data[top].len() && x < data[bottom].len() {
                            let tmp = data[top][x];
                            data[top][x] = data[bottom][x];
                            data[bottom][x] = tmp;
                        }
                    }
                }
                top += 1;
                bottom -= 1;
            }
        }

        state.set_glyph_pixels_internal(ch, data);
        Ok(())
    }

    // Helper for inverse glyph (self-reversing)
    fn do_inverse_glyph(&self, state: &mut BitFontEditState, ch: char) -> Result<()> {
        let data = state.get_glyph_pixels(ch).clone();
        let inverted: Vec<Vec<bool>> = data.iter().map(|row| row.iter().map(|&p| !p).collect()).collect();
        state.set_glyph_pixels_internal(ch, inverted);
        Ok(())
    }
}
