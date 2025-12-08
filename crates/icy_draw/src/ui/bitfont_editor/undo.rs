//! Undo operations for BitFont editor
//!
//! This module provides undo/redo functionality for the BitFont editor.
//! The trait is similar to `icy_engine_edit::UndoOperation` but operates
//! on `BitFontEditor` instead of `EditState`.

use i18n_embed_fl::fl;

use super::BitFontEditor;

/// Trait for BitFont editor undo operations
///
/// Similar to `icy_engine_edit::UndoOperation` but for BitFont-specific operations.
pub trait BitFontUndoOperation: Send {
    /// Get a description of this operation for display in the menu
    fn get_description(&self) -> String;

    /// Undo this operation
    fn undo(&mut self, editor: &mut BitFontEditor);

    /// Redo this operation
    fn redo(&mut self, editor: &mut BitFontEditor);
}

/// Edit glyph pixels operation
pub struct EditGlyph {
    ch: char,
    old_data: Vec<Vec<bool>>,
    new_data: Vec<Vec<bool>>,
}

impl EditGlyph {
    pub fn new(ch: char, old_data: Vec<Vec<bool>>, new_data: Vec<Vec<bool>>) -> Self {
        Self { ch, old_data, new_data }
    }
}

impl BitFontUndoOperation for EditGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-edit")
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        editor.set_glyph_pixels(self.ch, self.old_data.clone());
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        editor.set_glyph_pixels(self.ch, self.new_data.clone());
    }
}

/// Clear glyph operation
pub struct ClearGlyph {
    ch: char,
    old_data: Vec<Vec<bool>>,
}

impl ClearGlyph {
    pub fn new(ch: char) -> Self {
        Self {
            ch,
            old_data: Vec::new(),
        }
    }
}

impl BitFontUndoOperation for ClearGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-clear")
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        editor.set_glyph_pixels(self.ch, self.old_data.clone());
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        // Save old data before clearing
        self.old_data = editor.get_glyph_pixels(self.ch).clone();
        
        let (width, height) = editor.font_size();
        let cleared = vec![vec![false; width as usize]; height as usize];
        editor.set_glyph_pixels(self.ch, cleared);
    }
}

/// Inverse glyph operation
pub struct InverseGlyph {
    ch: char,
}

impl InverseGlyph {
    pub fn new(ch: char) -> Self {
        Self { ch }
    }
}

impl BitFontUndoOperation for InverseGlyph {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-inverse")
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        // Inverse is self-reversing
        self.redo(editor);
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        let data = editor.get_glyph_pixels(self.ch).clone();
        let inverted: Vec<Vec<bool>> = data
            .iter()
            .map(|row| row.iter().map(|&p| !p).collect())
            .collect();
        editor.set_glyph_pixels(self.ch, inverted);
    }
}

/// Move glyph operation
pub struct MoveGlyph {
    ch: char,
    dx: i32,
    dy: i32,
    old_data: Vec<Vec<bool>>,
}

impl MoveGlyph {
    pub fn new(ch: char, dx: i32, dy: i32) -> Self {
        Self {
            ch,
            dx,
            dy,
            old_data: Vec::new(),
        }
    }
}

impl BitFontUndoOperation for MoveGlyph {
    fn get_description(&self) -> String {
        match (self.dx, self.dy) {
            (0, -1) => fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-up"),
            (0, 1) => fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-down"),
            (-1, 0) => fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-left"),
            (1, 0) => fl!(crate::LANGUAGE_LOADER, "undo-bitfont-move-right"),
            _ => "Move Glyph".to_string(),
        }
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        editor.set_glyph_pixels(self.ch, self.old_data.clone());
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        self.old_data = editor.get_glyph_pixels(self.ch).clone();
        
        let (width, height) = editor.font_size();
        let mut new_data = vec![vec![false; width as usize]; height as usize];
        
        for y in 0..height as usize {
            for x in 0..width as usize {
                let src_x = x as i32 - self.dx;
                let src_y = y as i32 - self.dy;
                
                if src_x >= 0 && src_x < width && src_y >= 0 && src_y < height {
                    if let Some(row) = self.old_data.get(src_y as usize) {
                        if let Some(&pixel) = row.get(src_x as usize) {
                            new_data[y][x] = pixel;
                        }
                    }
                }
            }
        }
        
        editor.set_glyph_pixels(self.ch, new_data);
    }
}

/// Flip glyph operation
pub struct FlipGlyph {
    ch: char,
    horizontal: bool, // true = flip X, false = flip Y
}

impl FlipGlyph {
    pub fn new(ch: char, horizontal: bool) -> Self {
        Self { ch, horizontal }
    }
}

impl BitFontUndoOperation for FlipGlyph {
    fn get_description(&self) -> String {
        if self.horizontal {
            fl!(crate::LANGUAGE_LOADER, "undo-bitfont-flip-x")
        } else {
            fl!(crate::LANGUAGE_LOADER, "undo-bitfont-flip-y")
        }
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        // Flip is self-reversing
        self.redo(editor);
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        let data = editor.get_glyph_pixels(self.ch).clone();
        
        let flipped: Vec<Vec<bool>> = if self.horizontal {
            // Flip X: reverse each row
            data.iter()
                .map(|row| row.iter().rev().copied().collect())
                .collect()
        } else {
            // Flip Y: reverse row order
            data.into_iter().rev().collect()
        };
        
        editor.set_glyph_pixels(self.ch, flipped);
    }
}

/// Resize font operation
pub struct ResizeFont {
    old_width: i32,
    old_height: i32,
    new_width: i32,
    new_height: i32,
    old_glyph_data: Vec<Vec<Vec<bool>>>,
}

impl ResizeFont {
    pub fn new(old_width: i32, old_height: i32, new_width: i32, new_height: i32) -> Self {
        Self {
            old_width,
            old_height,
            new_width,
            new_height,
            old_glyph_data: Vec::new(),
        }
    }
}

impl BitFontUndoOperation for ResizeFont {
    fn get_description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "undo-bitfont-resize")
    }

    fn undo(&mut self, editor: &mut BitFontEditor) {
        // Restore old glyph data
        for (i, glyph_data) in self.old_glyph_data.iter().enumerate() {
            if let Some(ch) = char::from_u32(i as u32) {
                editor.set_glyph_pixels(ch, glyph_data.clone());
            }
        }
        
        // Restore old dimensions (the set_glyph_pixels won't resize, so we need to call resize)
        editor.resize_glyphs(self.old_width, self.old_height);
    }

    fn redo(&mut self, editor: &mut BitFontEditor) {
        // Save all glyph data before resize
        self.old_glyph_data.clear();
        for i in 0..256u32 {
            if let Some(ch) = char::from_u32(i) {
                self.old_glyph_data.push(editor.get_glyph_pixels(ch).clone());
            }
        }
        
        // Resize all glyphs
        editor.resize_glyphs(self.new_width, self.new_height);
    }
}
