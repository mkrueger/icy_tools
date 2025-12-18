//! BitFont Edit State
//!
//! The main state container for bitmap font editing. This separates the model
//! from the UI, following the same pattern as `EditState` for ANSI editing.
//!
//! # Architecture Overview
//!
//! The BitFont editor has two main panels:
//! - **Edit Grid (EditGrid)**: Shows the currently selected glyph's pixels in a large grid
//!   where users can edit individual pixels.
//! - **Character Set (CharSet)**: Shows all 256 characters in a 16×16 grid for navigation
//!   and multi-character operations.
//!
//! ## Focus and Context-Sensitive Behavior
//!
//! Many operations behave differently depending on which panel has focus:
//!
//! | Operation        | Edit Grid Focus                    | CharSet Focus                         |
//! |------------------|-----------------------------------|---------------------------------------|
//! | Clear/Erase      | Clears pixel selection (or all)   | Clears entire selected glyphs         |
//! | Fill             | Fills pixel selection (or all)    | Fills entire selected glyphs          |
//! | Inverse          | Inverts pixel selection (or all)  | Inverts entire selected glyphs        |
//! | Flip X/Y         | Flips pixel selection (or all)    | Flips each selected glyph entirely    |
//! | Slide            | Slides within current glyph       | Slides across all selected glyphs     |
//! | Move Glyph       | Moves pixels within glyph         | N/A (no multi-char move)              |
//! | Select All       | Selects all pixels in glyph       | Selects all 256 characters            |
//!
//! ## Selection Model
//!
//! ### Edit Selection (pixel-level)
//! - Rectangle selection within a single glyph's pixels
//! - Used for: Clear, Fill, Inverse, Flip operations when Edit Grid has focus
//! - If no selection exists, operations affect the entire glyph
//!
//! ### Charset Selection (character-level)
//! - Selects one or more characters in the 16×16 charset grid
//! - **Linear mode**: Characters are selected in reading order (left-to-right, top-to-bottom)
//! - **Rectangle mode** (Alt+drag): Characters are selected in a rectangular region
//! - Used for: Multi-character Clear, Fill, Inverse, Slide operations
//!
//! ## Cursor Movement (Wrapping Behavior)
//!
//! Both cursors wrap at boundaries using `rem_euclid`:
//!
//! - **Edit Grid Cursor**: Wraps within glyph bounds (0..font_width, 0..font_height)
//!   - Moving right from last column → wraps to column 0, same row
//!   - Moving down from last row → wraps to row 0, same column
//!   - X and Y wrap independently
//!
//! - **Charset Cursor**: Wraps within 16×16 grid (0..16, 0..16)
//!   - Same wrapping behavior as edit grid cursor
//!
//! Note: `set_cursor_pos()` and `set_charset_cursor()` use clamping, not wrapping.
//! Use `move_cursor()` and `move_charset_cursor()` for wrapped movement.
//!
//! ## Undo/Redo System
//!
//! All modifications go through the undo system:
//! - Single operations push one item to the undo stack
//! - Multi-character operations use `begin_atomic_undo()`/`end()` to group operations
//! - Atomic groups are undone/redone as a single unit
//! - The dirty flag tracks if the font has unsaved changes
//!
//! # Module Organization
//!
//! The implementation is split across multiple files matching the test structure:
//! - `state.rs` - Struct definition, constructors, getters, basic setters
//! - `cursor.rs` - Cursor movement (edit grid and charset)
//! - `selection.rs` - Selection handling (edit and charset)
//! - `glyph_operations.rs` - Single glyph operations (pixel, clear, flip, inverse, move, slide)
//! - `charset_operations.rs` - Context-sensitive multi-glyph operations (fill, erase, inverse)
//! - `font_operations.rs` - Font-level operations (resize, insert/delete line/column)
//! - `clipboard.rs` - Copy, cut, paste
//! - `undo.rs` - Undo/redo system
//! - `internal.rs` - Internal setters for undo operations

use std::path::PathBuf;

use icy_engine::{BitFont, Position, Selection};

use crate::Result;
use crate::bitfont::undo_stack::BitFontUndoStack;

use super::BitFontFocusedPanel;

// ═══════════════════════════════════════════════════════════════════════════
// BitFont Edit State
// ═══════════════════════════════════════════════════════════════════════════

/// Main state container for bitmap font editing
///
/// This struct contains all the model data for a font being edited:
/// - Glyph pixel data (256 glyphs)
/// - Font dimensions
/// - Selection state
/// - Cursor positions
/// - Undo/redo stacks
///
/// The UI layer should only read from this state and call methods to modify it.
/// All modifications go through the undo system.
pub struct BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Font Data
    // ═══════════════════════════════════════════════════════════════════════
    /// Editable glyph data: 256 glyphs, each as Vec<Vec<bool>> (height × width)
    pub(crate) glyph_data: Vec<Vec<Vec<bool>>>,

    /// Font width in pixels
    pub(crate) font_width: i32,

    /// Font height in pixels
    pub(crate) font_height: i32,

    /// Font name
    pub(crate) font_name: String,

    // ═══════════════════════════════════════════════════════════════════════
    // Selection & Cursor
    // ═══════════════════════════════════════════════════════════════════════
    /// Currently selected character (0-255)
    pub(crate) selected_char: char,

    /// Cursor position in the edit grid (x, y)
    pub(crate) cursor_pos: (i32, i32),

    /// Rectangular selection in the edit grid (for pixel selection)
    /// Uses anchor/lead positions - always Rectangle shape
    pub(crate) edit_selection: Option<Selection>,

    /// Cursor position in the character set grid (0-15, 0-15 for 16x16 grid)
    pub(crate) charset_cursor: (i32, i32),

    /// Selection in the character set grid for multi-character operations
    /// Uses anchor/lead positions: (anchor, lead, is_rectangle)
    /// - Anchor: where selection started
    /// - Lead: current cursor position
    /// - is_rectangle: false = linear selection (default), true = rectangle (Alt+drag)
    pub(crate) charset_selection: Option<(Position, Position, bool)>,

    // ═══════════════════════════════════════════════════════════════════════
    // Focus State
    // ═══════════════════════════════════════════════════════════════════════
    /// Which panel currently has focus
    pub(crate) focused_panel: BitFontFocusedPanel,

    // ═══════════════════════════════════════════════════════════════════════
    // File State
    // ═══════════════════════════════════════════════════════════════════════
    /// File path (if loaded from/saved to file)
    pub(crate) file_path: Option<PathBuf>,

    /// Whether the font has been modified since last save
    pub(crate) is_dirty: bool,

    // ═══════════════════════════════════════════════════════════════════════
    // Display Options
    // ═══════════════════════════════════════════════════════════════════════
    /// Whether to use 9-dot cell mode (VGA letter spacing)
    pub(crate) use_letter_spacing: bool,

    // ═══════════════════════════════════════════════════════════════════════
    // Undo System
    // ═══════════════════════════════════════════════════════════════════════
    /// Undo stack (serializable)
    pub(crate) undo_stack: BitFontUndoStack,
}

impl Default for BitFontEditState {
    fn default() -> Self {
        Self::new()
    }
}

impl BitFontEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Constructors
    // ═══════════════════════════════════════════════════════════════════════

    /// Create a new BitFontEditState with a default 8x16 font
    pub fn new() -> Self {
        let font = BitFont::default();
        Self::from_font(font)
    }

    /// Create a BitFontEditState from an existing BitFont
    pub fn from_font(font: BitFont) -> Self {
        let size = font.size();
        let glyph_data = Self::extract_glyph_data(&font, size.width, size.height);
        let font_name = font.name().to_string();

        Self {
            glyph_data,
            font_width: size.width,
            font_height: size.height,
            font_name,
            selected_char: 'A',
            cursor_pos: (0, 0),
            edit_selection: None,
            charset_cursor: (0, 4), // 'A' = 65 = 4*16+1, so row 4, col 1
            charset_selection: None,
            focused_panel: BitFontFocusedPanel::EditGrid,
            file_path: None,
            is_dirty: false,
            use_letter_spacing: false,
            undo_stack: BitFontUndoStack::new(),
        }
    }

    /// Create a BitFontEditState from a file
    pub fn from_file(path: PathBuf) -> Result<Self> {
        let data = std::fs::read(&path).map_err(|e| crate::EngineError::Generic(format!("Failed to read file: {}", e)))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();
        let font = BitFont::from_bytes(name, &data).map_err(|e| crate::EngineError::Generic(format!("Failed to parse font: {}", e)))?;

        let mut state = Self::from_font(font);
        state.file_path = Some(path);
        Ok(state)
    }

    /// Extract all glyph pixel data from a BitFont
    fn extract_glyph_data(font: &BitFont, width: i32, height: i32) -> Vec<Vec<Vec<bool>>> {
        let mut glyphs = Vec::with_capacity(256);

        for ch_code in 0..256u32 {
            let ch = char::from_u32(ch_code).unwrap_or(' ');
            let mut pixels = vec![vec![false; width as usize]; height as usize];

            if let Some(glyph) = font.glyph(ch) {
                for (y, row) in glyph.bitmap.pixels.iter().enumerate() {
                    if y >= height as usize {
                        break;
                    }
                    for (x, &pixel) in row.iter().enumerate() {
                        if x >= width as usize {
                            break;
                        }
                        pixels[y][x] = pixel;
                    }
                }
            }

            glyphs.push(pixels);
        }

        glyphs
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Getters
    // ═══════════════════════════════════════════════════════════════════════

    /// Get font dimensions (width, height)
    pub fn font_size(&self) -> (i32, i32) {
        (self.font_width, self.font_height)
    }

    /// Get font width
    pub fn font_width(&self) -> i32 {
        self.font_width
    }

    /// Get font height
    pub fn font_height(&self) -> i32 {
        self.font_height
    }

    /// Get font name
    pub fn font_name(&self) -> &str {
        &self.font_name
    }

    /// Get currently selected character
    pub fn selected_char(&self) -> char {
        self.selected_char
    }

    /// Get currently focused panel
    pub fn focused_panel(&self) -> BitFontFocusedPanel {
        self.focused_panel
    }

    /// Get file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Check if font is dirty (modified)
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Check if letter spacing (9-dot mode) is enabled
    pub fn use_letter_spacing(&self) -> bool {
        self.use_letter_spacing
    }

    /// Get pixel data for a character (read-only)
    pub fn get_glyph_pixels(&self, ch: char) -> &Vec<Vec<bool>> {
        let idx = (ch as u32).min(255) as usize;
        &self.glyph_data[idx]
    }

    /// Get all glyph data (for preview/export)
    pub fn get_all_glyph_data(&self) -> &Vec<Vec<Vec<bool>>> {
        &self.glyph_data
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Basic Setters (non-undoable, for UI state)
    // ═══════════════════════════════════════════════════════════════════════

    /// Set selected character
    pub fn set_selected_char(&mut self, ch: char) {
        self.selected_char = ch;
        // Update charset cursor to match
        let code = ch as u32;
        self.charset_cursor = ((code % 16) as i32, (code / 16) as i32);
    }

    /// Set file path
    pub fn set_file_path(&mut self, path: Option<PathBuf>) {
        self.file_path = path;
    }

    /// Mark as clean (after save)
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }

    /// Toggle letter spacing mode
    pub fn toggle_letter_spacing(&mut self) {
        self.use_letter_spacing = !self.use_letter_spacing;
    }

    /// Set letter spacing mode
    pub fn set_letter_spacing(&mut self, enabled: bool) {
        self.use_letter_spacing = enabled;
    }

    /// Set focused panel
    pub fn set_focused_panel(&mut self, panel: BitFontFocusedPanel) {
        self.focused_panel = panel;
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Export
    // ═══════════════════════════════════════════════════════════════════════

    /// Build a BitFont from current glyph data
    pub fn build_font(&self) -> BitFont {
        // Convert glyph_data to raw bytes format
        let mut raw_data: Vec<u8> = Vec::with_capacity(256 * self.font_height as usize);

        for glyph in &self.glyph_data {
            for row in glyph {
                let mut byte = 0u8;
                for (x, &pixel) in row.iter().enumerate() {
                    if pixel && x < 8 {
                        byte |= 1 << (7 - x);
                    }
                }
                raw_data.push(byte);
            }
        }

        // Create font from raw data
        BitFont::create_8(self.font_name.clone(), self.font_width as u8, self.font_height as u8, &raw_data)
    }
}
