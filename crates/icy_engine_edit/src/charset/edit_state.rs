//! CharSet Edit State
//!
//! The main state container for TheDraw Font (TDF) editing. This separates the model
//! from the UI, following the same pattern as `BitFontEditState` for BitFont editing.
//!
//! # Architecture Overview
//!
//! The CharSet editor has multiple panels:
//! - **Character Preview**: Shows the currently selected character rendered at zoom
//! - **Character Set Grid**: Shows printable ASCII characters (! to ~) in a 16×6 grid
//! - **Font List**: Shows all fonts in the TDF file
//! - **Color/Tool Palette**: For color font editing
//!
//! ## Focus and Context-Sensitive Behavior
//!
//! Operations behave differently depending on which panel has focus:
//!
//! | Operation        | Edit Focus                        | CharSet Focus                         |
//! |------------------|-----------------------------------|---------------------------------------|
//! | Clear            | Clears current glyph              | Clears selected glyphs                |
//! | Arrow Keys       | Edit cursor movement              | CharSet cursor movement               |
//! | Enter/Space      | N/A                               | Select character for editing          |
//!
//! ## Selection Model
//!
//! ### Charset Selection (character-level)
//! - Selects one or more characters in the 16×6 charset grid
//! - Characters range from '!' (0x21) to '~' (0x7E) = 94 characters
//! - Grid is 16 columns × 6 rows (last row is partial)

use std::path::PathBuf;

use icy_engine::Position;
use retrofont::{
    tdf::{TdfFont, TdfFontType},
    Glyph,
};

use super::{load_tdf_fonts_from_file, CharSetUndoOperation, CharSetUndoStack};

/// Which panel currently has focus in the CharSet editor
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CharSetFocusedPanel {
    /// The character editing canvas
    Edit,
    /// The character set grid
    #[default]
    CharSet,
}

/// Main state container for CharSet (TDF) font editing
///
/// This struct contains all the model data for TDF fonts being edited,
/// using `retrofont::tdf::TdfFont` directly.
pub struct CharSetEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Font Data
    // ═══════════════════════════════════════════════════════════════════════
    /// The TDF fonts being edited (using retrofont directly)
    fonts: Vec<TdfFont>,

    /// Currently selected font index
    selected_font: usize,

    // ═══════════════════════════════════════════════════════════════════════
    // Selection & Cursor
    // ═══════════════════════════════════════════════════════════════════════
    /// Currently selected character for editing
    selected_char: Option<char>,

    /// Previous selected character (for saving on change)
    old_selected_char: Option<char>,

    /// Cursor position in the character set grid (0-15, 0-5)
    charset_cursor: (i32, i32),

    /// Selection in the character set grid
    /// Uses anchor/lead positions: (anchor, lead, is_rectangle)
    charset_selection: Option<(Position, Position, bool)>,

    // ═══════════════════════════════════════════════════════════════════════
    // Focus State
    // ═══════════════════════════════════════════════════════════════════════
    /// Which panel currently has focus
    focused_panel: CharSetFocusedPanel,

    // ═══════════════════════════════════════════════════════════════════════
    // File State
    // ═══════════════════════════════════════════════════════════════════════
    /// File path (if loaded from/saved to file)
    file_path: Option<PathBuf>,

    /// Whether the font has been modified since last save
    is_dirty: bool,

    // ═══════════════════════════════════════════════════════════════════════
    // Undo/Redo
    // ═══════════════════════════════════════════════════════════════════════
    /// Undo stack
    undo_stack: CharSetUndoStack,
}

impl Default for CharSetEditState {
    fn default() -> Self {
        Self::new()
    }
}

impl CharSetEditState {
    // ═══════════════════════════════════════════════════════════════════════
    // Constructors
    // ═══════════════════════════════════════════════════════════════════════

    /// Create a new empty CharSet edit state with a default Color font
    pub fn new() -> Self {
        use retrofont::tdf::TdfFontType;
        Self::new_with_font_type(TdfFontType::Color)
    }

    /// Create a new empty CharSet edit state with the specified font type
    pub fn new_with_font_type(font_type: retrofont::tdf::TdfFontType) -> Self {
        let fonts = vec![TdfFont::new("New Font", font_type, 1)];
        Self {
            fonts,
            selected_font: 0,
            selected_char: None,
            old_selected_char: None,
            charset_cursor: (0, 0),
            charset_selection: None,
            focused_panel: CharSetFocusedPanel::CharSet,
            file_path: None,
            is_dirty: false,
            undo_stack: CharSetUndoStack::new(),
        }
    }

    /// Create from existing fonts
    pub fn with_fonts(fonts: Vec<TdfFont>, file_path: Option<PathBuf>) -> Self {
        use retrofont::tdf::TdfFontType;
        let fonts = if fonts.is_empty() {
            vec![TdfFont::new("New Font", TdfFontType::Color, 1)]
        } else {
            fonts
        };

        Self {
            fonts,
            selected_font: 0,
            selected_char: None,
            old_selected_char: None,
            charset_cursor: (0, 0),
            charset_selection: None,
            focused_panel: CharSetFocusedPanel::CharSet,
            file_path,
            is_dirty: false,
            undo_stack: CharSetUndoStack::new(),
        }
    }

    /// Load from a TDF file
    pub fn load_from_file(path: PathBuf) -> anyhow::Result<Self> {
        let fonts = load_tdf_fonts_from_file(&path)?;
        Ok(Self::with_fonts(fonts, Some(path)))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Getters
    // ═══════════════════════════════════════════════════════════════════════

    /// Get all fonts
    pub fn fonts(&self) -> &[TdfFont] {
        &self.fonts
    }

    /// Get mutable access to all fonts
    pub fn fonts_mut(&mut self) -> &mut Vec<TdfFont> {
        &mut self.fonts
    }

    /// Get the currently selected font
    pub fn selected_font(&self) -> Option<&TdfFont> {
        self.fonts.get(self.selected_font)
    }

    /// Get mutable access to the currently selected font
    pub fn selected_font_mut(&mut self) -> Option<&mut TdfFont> {
        self.fonts.get_mut(self.selected_font)
    }

    /// Get the selected font index
    pub fn selected_font_index(&self) -> usize {
        self.selected_font
    }

    /// Get the number of fonts
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Get the currently selected character
    pub fn selected_char(&self) -> Option<char> {
        self.selected_char
    }

    /// Get the charset cursor position
    pub fn charset_cursor(&self) -> (i32, i32) {
        self.charset_cursor
    }

    /// Get the charset selection
    pub fn charset_selection(&self) -> Option<(Position, Position, bool)> {
        self.charset_selection
    }

    /// Get the focused panel
    pub fn focused_panel(&self) -> CharSetFocusedPanel {
        self.focused_panel
    }

    /// Get the file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Check if the state is dirty (has unsaved changes)
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Get undo stack length
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.undo_len()
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Get undo description
    pub fn undo_description(&self) -> Option<String> {
        self.undo_stack.undo_description()
    }

    /// Get redo description
    pub fn redo_description(&self) -> Option<String> {
        self.undo_stack.redo_description()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Setters
    // ═══════════════════════════════════════════════════════════════════════

    /// Set the file path
    pub fn set_file_path(&mut self, path: Option<PathBuf>) {
        self.file_path = path;
    }

    /// Set the dirty flag
    pub fn set_dirty(&mut self, dirty: bool) {
        self.is_dirty = dirty;
    }

    /// Set the focused panel
    pub fn set_focused_panel(&mut self, panel: CharSetFocusedPanel) {
        self.focused_panel = panel;
    }

    /// Select a font by index
    pub fn select_font(&mut self, index: usize) {
        if index < self.fonts.len() && index != self.selected_font {
            self.selected_font = index;
            self.selected_char = None;
            self.old_selected_char = None;
        }
    }

    /// Select a character for editing
    pub fn select_char(&mut self, ch: char) {
        self.selected_char = Some(ch);
        // Update cursor position based on character
        if let Some((col, row)) = char_to_grid(ch) {
            self.charset_cursor = (col, row);
        }
        self.charset_selection = None;
    }

    /// Select a character at a specific grid position
    pub fn select_char_at(&mut self, col: i32, row: i32) {
        if let Some(ch) = grid_to_char(col, row) {
            self.selected_char = Some(ch);
            self.charset_cursor = (col, row);
            self.charset_selection = Some((Position::new(col, row), Position::new(col, row), false));
            self.focused_panel = CharSetFocusedPanel::CharSet;
        }
    }

    /// Set the charset cursor position
    pub fn set_charset_cursor(&mut self, col: i32, row: i32) {
        self.charset_cursor = (col.clamp(0, 15), row.clamp(0, 5));
    }

    /// Clear the charset selection
    pub fn clear_charset_selection(&mut self) {
        self.charset_selection = None;
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Cursor Movement
    // ═══════════════════════════════════════════════════════════════════════

    /// Move the charset cursor by delta, clamping at boundaries
    pub fn move_charset_cursor(&mut self, dx: i32, dy: i32) {
        let (col, row) = self.charset_cursor;
        self.charset_cursor = ((col + dx).clamp(0, 15), (row + dy).clamp(0, 5));
        self.charset_selection = None;
    }

    /// Extend charset selection with shift+arrows
    pub fn extend_charset_selection(&mut self, dx: i32, dy: i32, is_rectangle: bool) {
        let (col, row) = self.charset_cursor;
        let new_col = (col + dx).clamp(0, 15);
        let new_row = (row + dy).clamp(0, 5);
        self.charset_cursor = (new_col, new_row);

        if let Some((anchor, _, _)) = self.charset_selection {
            self.charset_selection = Some((anchor, Position::new(new_col, new_row), is_rectangle));
        } else {
            // Start new selection from previous cursor position
            self.charset_selection = Some((Position::new(col, row), Position::new(new_col, new_row), is_rectangle));
        }
    }

    /// Move charset cursor to home (start of row)
    pub fn charset_home(&mut self) {
        self.charset_cursor = (0, self.charset_cursor.1);
        self.charset_selection = None;
    }

    /// Move charset cursor to end (end of row)
    pub fn charset_end(&mut self) {
        self.charset_cursor = (15, self.charset_cursor.1);
        self.charset_selection = None;
    }

    /// Move charset cursor to top (first row)
    pub fn charset_page_up(&mut self) {
        self.charset_cursor = (self.charset_cursor.0, 0);
        self.charset_selection = None;
    }

    /// Move charset cursor to bottom (last row)
    pub fn charset_page_down(&mut self) {
        self.charset_cursor = (self.charset_cursor.0, 5);
        self.charset_selection = None;
    }

    /// Select character at current cursor position
    pub fn select_char_at_cursor(&mut self) {
        let (col, row) = self.charset_cursor;
        if let Some(ch) = grid_to_char(col, row) {
            self.selected_char = Some(ch);
        }
    }

    /// Toggle focus between panels
    pub fn focus_next_panel(&mut self) {
        self.focused_panel = match self.focused_panel {
            CharSetFocusedPanel::Edit => CharSetFocusedPanel::CharSet,
            CharSetFocusedPanel::CharSet => CharSetFocusedPanel::Edit,
        };
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Font Operations
    // ═══════════════════════════════════════════════════════════════════════

    /// Add a new font with the given type, name, and spacing
    pub fn add_font(&mut self, font_type: TdfFontType, name: String, spacing: i32) {
        let new_font = TdfFont::new(name, font_type, spacing);
        let new_index = self.fonts.len();

        self.undo_stack.push(CharSetUndoOperation::FontAdded {
            font_index: new_index,
            font: new_font.clone(),
        });

        self.fonts.push(new_font);
        self.selected_font = new_index;
        self.selected_char = None;
        self.old_selected_char = None;
        self.is_dirty = true;
    }

    /// Clone the current font
    pub fn clone_font(&mut self) {
        if self.selected_font < self.fonts.len() {
            let cloned = self.fonts[self.selected_font].clone();
            let new_index = self.fonts.len();

            self.undo_stack.push(CharSetUndoOperation::FontAdded {
                font_index: new_index,
                font: cloned.clone(),
            });

            self.fonts.push(cloned);
            self.selected_font = new_index;
            self.selected_char = None;
            self.old_selected_char = None;
            self.is_dirty = true;
        }
    }

    /// Delete the current font (if more than one exists)
    pub fn delete_font(&mut self) {
        if self.fonts.len() > 1 && self.selected_font < self.fonts.len() {
            let removed = self.fonts.remove(self.selected_font);

            self.undo_stack.push(CharSetUndoOperation::FontRemoved {
                font_index: self.selected_font,
                font: removed,
            });

            self.selected_font = 0;
            self.selected_char = None;
            self.old_selected_char = None;
            self.is_dirty = true;
        }
    }

    /// Move the current font up in the list
    pub fn move_font_up(&mut self) {
        if self.selected_font > 0 && self.selected_font < self.fonts.len() {
            self.fonts.swap(self.selected_font, self.selected_font - 1);
            self.selected_font -= 1;
            self.is_dirty = true;
        }
    }

    /// Move the current font down in the list
    pub fn move_font_down(&mut self) {
        if self.selected_font + 1 < self.fonts.len() {
            self.fonts.swap(self.selected_font, self.selected_font + 1);
            self.selected_font += 1;
            self.is_dirty = true;
        }
    }

    /// Set font name
    pub fn set_font_name(&mut self, name: String) {
        if self.selected_font < self.fonts.len() {
            let old_name = self.fonts[self.selected_font].name.clone();
            if old_name != name {
                self.undo_stack.push(CharSetUndoOperation::FontNameChanged {
                    font_index: self.selected_font,
                    old_name,
                    new_name: name.clone(),
                });
                self.fonts[self.selected_font].name = name;
                self.is_dirty = true;
            }
        }
    }

    /// Set font spacing
    pub fn set_font_spacing(&mut self, spacing: i32) {
        if self.selected_font < self.fonts.len() {
            let old_spacing = self.fonts[self.selected_font].spacing;
            if old_spacing != spacing {
                self.undo_stack.push(CharSetUndoOperation::FontSpacingChanged {
                    font_index: self.selected_font,
                    old_spacing,
                    new_spacing: spacing,
                });
                self.fonts[self.selected_font].spacing = spacing;
                self.is_dirty = true;
            }
        }
    }

    /// Check if a character has a glyph
    pub fn has_glyph(&self, ch: char) -> bool {
        self.fonts.get(self.selected_font).map(|f| f.has_char(ch)).unwrap_or(false)
    }

    /// Get a glyph for a character
    pub fn get_glyph(&self, ch: char) -> Option<&Glyph> {
        self.fonts.get(self.selected_font).and_then(|f| f.glyph(ch))
    }

    /// Clear the currently selected character's glyph
    pub fn clear_selected_char(&mut self) {
        if let Some(ch) = self.selected_char {
            if self.selected_font < self.fonts.len() {
                let old_glyph = self.fonts[self.selected_font].glyph(ch).cloned();

                self.undo_stack.push(CharSetUndoOperation::GlyphModified {
                    font_index: self.selected_font,
                    char_code: ch,
                    old_glyph,
                    new_glyph: None,
                });

                self.fonts[self.selected_font].remove_glyph(ch);
                self.is_dirty = true;
            }
        }
    }

    /// Delete all selected characters in the charset grid
    /// If there's a selection, delete all characters in the selection.
    /// Otherwise, delete the character at the cursor position.
    pub fn delete_selected_chars(&mut self) {
        if self.selected_font >= self.fonts.len() {
            return;
        }

        // Collect all characters to delete
        let chars_to_delete: Vec<char> = if let Some((anchor, lead, is_rectangle)) = self.charset_selection {
            self.get_selected_chars(anchor, lead, is_rectangle)
        } else {
            // No selection - delete character at cursor
            if let Some(ch) = grid_to_char(self.charset_cursor.0, self.charset_cursor.1) {
                vec![ch]
            } else {
                vec![]
            }
        };

        if chars_to_delete.is_empty() {
            return;
        }

        // Begin atomic undo for multiple deletions
        if chars_to_delete.len() > 1 {
            self.undo_stack.begin_atomic();
        }

        for ch in chars_to_delete {
            let old_glyph = self.fonts[self.selected_font].glyph(ch).cloned();
            if old_glyph.is_some() {
                self.undo_stack.push(CharSetUndoOperation::GlyphModified {
                    font_index: self.selected_font,
                    char_code: ch,
                    old_glyph,
                    new_glyph: None,
                });
                self.fonts[self.selected_font].remove_glyph(ch);
            }
        }

        if self.undo_stack.is_in_atomic() {
            self.undo_stack.end_atomic();
        }

        self.is_dirty = true;
        self.charset_selection = None;
    }

    /// Get all characters in the current selection
    fn get_selected_chars(&self, anchor: Position, lead: Position, is_rectangle: bool) -> Vec<char> {
        let mut chars = Vec::new();
        let min_col = anchor.x.min(lead.x);
        let max_col = anchor.x.max(lead.x);
        let min_row = anchor.y.min(lead.y);
        let max_row = anchor.y.max(lead.y);

        if is_rectangle {
            // Rectangle selection
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    if let Some(ch) = grid_to_char(col, row) {
                        chars.push(ch);
                    }
                }
            }
        } else {
            // Linear selection (reading order)
            let start_idx = anchor.y * 16 + anchor.x;
            let end_idx = lead.y * 16 + lead.x;
            let (start, end) = if start_idx <= end_idx { (start_idx, end_idx) } else { (end_idx, start_idx) };
            for idx in start..=end {
                let col = idx % 16;
                let row = idx / 16;
                if let Some(ch) = grid_to_char(col, row) {
                    chars.push(ch);
                }
            }
        }
        chars
    }

    /// Set a glyph for a character
    pub fn set_glyph(&mut self, ch: char, glyph: Glyph) {
        if self.selected_font < self.fonts.len() {
            let old_glyph = self.fonts[self.selected_font].glyph(ch).cloned();

            self.undo_stack.push(CharSetUndoOperation::GlyphModified {
                font_index: self.selected_font,
                char_code: ch,
                old_glyph,
                new_glyph: Some(glyph.clone()),
            });

            self.fonts[self.selected_font].add_glyph(ch, glyph);
            self.is_dirty = true;
        }
    }

    /// Clear/remove a glyph for a specific character
    pub fn clear_glyph(&mut self, ch: char) {
        if self.selected_font < self.fonts.len() {
            let old_glyph = self.fonts[self.selected_font].glyph(ch).cloned();
            if old_glyph.is_none() {
                return; // Nothing to clear
            }

            self.undo_stack.push(CharSetUndoOperation::GlyphModified {
                font_index: self.selected_font,
                char_code: ch,
                old_glyph,
                new_glyph: None,
            });

            self.fonts[self.selected_font].remove_glyph(ch);
            self.is_dirty = true;
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Undo/Redo
    // ═══════════════════════════════════════════════════════════════════════

    /// Undo the last operation
    pub fn undo(&mut self) -> bool {
        if let Some(op) = self.undo_stack.pop_undo() {
            self.apply_undo_operation(&op, true);
            self.undo_stack.push_redo(op);
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) -> bool {
        if let Some(op) = self.undo_stack.pop_redo() {
            self.apply_undo_operation(&op, false);
            self.undo_stack.push(op);
            true
        } else {
            false
        }
    }

    /// Apply an undo operation (reverse if is_undo, forward if is_redo)
    fn apply_undo_operation(&mut self, op: &CharSetUndoOperation, is_undo: bool) {
        match op {
            CharSetUndoOperation::GlyphModified {
                font_index,
                char_code,
                old_glyph,
                new_glyph,
            } => {
                if *font_index < self.fonts.len() {
                    let glyph = if is_undo { old_glyph } else { new_glyph };
                    if let Some(g) = glyph {
                        self.fonts[*font_index].add_glyph(*char_code, g.clone());
                    } else {
                        self.fonts[*font_index].remove_glyph(*char_code);
                    }
                }
            }
            CharSetUndoOperation::FontNameChanged {
                font_index,
                old_name,
                new_name,
            } => {
                if *font_index < self.fonts.len() {
                    self.fonts[*font_index].name = if is_undo { old_name.clone() } else { new_name.clone() };
                }
            }
            CharSetUndoOperation::FontSpacingChanged {
                font_index,
                old_spacing,
                new_spacing,
            } => {
                if *font_index < self.fonts.len() {
                    let spacing = if is_undo { *old_spacing } else { *new_spacing };
                    self.fonts[*font_index].spacing = spacing;
                }
            }
            CharSetUndoOperation::FontAdded { font_index, font } => {
                if is_undo {
                    if *font_index < self.fonts.len() {
                        self.fonts.remove(*font_index);
                        if self.selected_font >= self.fonts.len() {
                            self.selected_font = self.fonts.len().saturating_sub(1);
                        }
                    }
                } else {
                    self.fonts.insert(*font_index, font.clone());
                }
            }
            CharSetUndoOperation::FontRemoved { font_index, font } => {
                if is_undo {
                    self.fonts.insert(*font_index, font.clone());
                } else {
                    if *font_index < self.fonts.len() {
                        self.fonts.remove(*font_index);
                        if self.selected_font >= self.fonts.len() {
                            self.selected_font = self.fonts.len().saturating_sub(1);
                        }
                    }
                }
            }
            CharSetUndoOperation::AtomicStart | CharSetUndoOperation::AtomicEnd => {
                // Markers, no action needed
            }
        }
    }

    /// Begin an atomic undo group
    pub fn begin_atomic_undo(&mut self) {
        self.undo_stack.begin_atomic();
    }

    /// End an atomic undo group
    pub fn end_atomic_undo(&mut self) {
        self.undo_stack.end_atomic();
    }

    /// Clear the undo stack
    pub fn clear_undo_stack(&mut self) {
        self.undo_stack.clear();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Save/Load
    // ═══════════════════════════════════════════════════════════════════════

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bytes = TdfFont::serialize_bundle(&self.fonts).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        self.file_path = Some(path.to_path_buf());
        self.is_dirty = false;
        Ok(())
    }

    /// Get bytes for autosave
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        TdfFont::serialize_bundle(&self.fonts).map_err(|e| e.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Map from grid position to character code
/// Grid is 16 columns x 6 rows for chars '!' (0x21) to '~' (0x7E)
pub fn grid_to_char(col: i32, row: i32) -> Option<char> {
    if col < 0 || col >= 16 || row < 0 || row >= 6 {
        return None;
    }
    let index = row * 16 + col;
    let ch_code = b'!' + index as u8;
    if ch_code <= b'~' {
        Some(ch_code as char)
    } else {
        None
    }
}

/// Map from character to grid position
pub fn char_to_grid(ch: char) -> Option<(i32, i32)> {
    let code = ch as u8;
    if code >= b'!' && code <= b'~' {
        let index = (code - b'!') as i32;
        Some((index % 16, index / 16))
    } else {
        None
    }
}
