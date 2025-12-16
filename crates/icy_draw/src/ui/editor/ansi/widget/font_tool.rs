//! Font Tool state for TDF/Figlet font rendering
//!
//! This is the per-editor state that references the shared FontLibrary.

use crate::SharedFontLibrary;

/// Font tool state for each editor
///
/// This holds a reference to the shared font library plus per-editor state
/// like the currently selected font and kerning state.
pub struct FontToolState {
    /// Reference to the shared font library
    pub font_library: SharedFontLibrary,
    /// Currently selected font index (-1 = none)
    pub selected_font: i32,
    /// Previous character typed (for kerning)
    pub prev_char: char,
}

impl FontToolState {
    /// Create a new font tool state with a reference to the shared library
    pub fn new(font_library: SharedFontLibrary) -> Self {
        // Auto-select first font if available
        let selected_font = {
            let lib = font_library.read();
            if lib.has_fonts() { 0 } else { -1 }
        };

        Self {
            font_library,
            selected_font,
            prev_char: '\0',
        }
    }

    /// Execute a function with the selected font if available
    /// This avoids cloning the font which doesn't implement Clone
    pub fn with_selected_font<T, F: FnOnce(&retrofont::Font) -> T>(&self, f: F) -> Option<T> {
        let lib = self.font_library.read();
        if self.selected_font >= 0 && (self.selected_font as usize) < lib.font_count() {
            lib.get_font(self.selected_font as usize).map(f)
        } else {
            None
        }
    }

    /// Execute a function with a font at a specific index
    pub fn with_font_at<T, F: FnOnce(&retrofont::Font) -> T>(&self, index: usize, f: F) -> Option<T> {
        let lib = self.font_library.read();
        lib.get_font(index).map(f)
    }

    /// Check if any fonts are loaded
    pub fn has_fonts(&self) -> bool {
        self.font_library.read().has_fonts()
    }

    /// Get access to the shared font library
    pub fn font_library(&self) -> SharedFontLibrary {
        self.font_library.clone()
    }

    /// Get the number of loaded fonts
    pub fn font_count(&self) -> usize {
        self.font_library.read().font_count()
    }

    /// Select font by index
    pub fn select_font(&mut self, index: i32) {
        let count = self.font_library.read().font_count();
        if index >= 0 && (index as usize) < count {
            self.selected_font = index;
            self.prev_char = '\0'; // Reset kerning state
        }
    }

    /// Check if current font has a glyph for the character
    pub fn has_char(&self, ch: char) -> bool {
        if self.selected_font < 0 {
            return false;
        }
        self.font_library.read().has_char(self.selected_font as usize, ch)
    }

    /// Get the maximum height of glyphs in the selected font
    pub fn max_height(&self) -> usize {
        self.with_selected_font(|font| font.max_height()).unwrap_or(1)
    }
}
