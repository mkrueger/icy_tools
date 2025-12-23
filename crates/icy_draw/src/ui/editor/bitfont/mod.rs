//! BitFont Editor for icy_draw
//!
//! Provides a pixel-based editor for bitmap fonts (.psf, .fXX, .yaff files).
//! Features:
//! - Glyph selector grid (256 characters)
//! - Pixel edit grid with click/drag drawing
//! - Toolbar with Clear, Inverse, Move, Flip operations
//! - Resize font dimensions
//! - Undo/Redo support
//! - Tool system: Click, Select, Rectangle, Fill
//! - Keyboard cursor navigation
//! - Save as .yaff format (human-readable)

mod canvas;
mod font_size_dialog;
mod left_panel;
pub mod menu_bar;
mod messages;
pub mod style;
mod tile_view;
mod tools;
mod top_toolbar;

pub use canvas::*;
pub use font_size_dialog::*;
pub use left_panel::*;
pub use messages::*;
pub use tools::*;
pub use top_toolbar::*;

use std::{path::PathBuf, sync::Arc};

use codepages::tables::CP437_TO_UNICODE;
use iced::{
    alignment::Horizontal,
    keyboard::{self, Key},
    mouse,
    widget::{column, container, row, text, Canvas, Space},
    Element, Length, Point, Task, Theme,
};
use icy_engine::BitFont;
use icy_engine_edit::bitfont::{BitFontEditState, BitFontFocusedPanel, BitFontUndoState};
use icy_engine_gui::{
    theme::{self, main_area_background},
    ui::DialogStack,
    MonitorSettings, Terminal, TerminalView,
};
use parking_lot::Mutex;

use crate::ui::main_window::Message;
use crate::ui::{
    editor::ansi::{constants::SIDEBAR_WIDTH, PaletteGrid, PaletteGridMessage, SWITCHER_SIZE},
    editor::bitfont::tile_view::TileViewCanvas,
};

// Layout constants are now in style.rs - import them here for backwards compatibility
pub(crate) use style::{CELL_GAP as EDIT_CELL_BORDER, CELL_SIZE as EDIT_CELL_SIZE, RULER_SIZE};

/// Maximum height available for the grids (accounting for toolbar, margins, etc.)
const MAX_GRID_HEIGHT: f32 = 580.0;
/// Minimum scale factor to prevent grids from becoming too small
const MIN_SCALE_FACTOR: f32 = 0.25;

/// State for the BitFont editor
///
/// We store our own editable glyph data since BitFont doesn't expose mutable access
/// to glyph pixels directly. When saving, we'll convert back to yaff format.
pub struct BitFontEditor {
    /// Backend model containing all font editing state and undo history
    pub(crate) state: BitFontEditState,
    /// Target width for resize UI
    target_width: i32,
    /// Target height for resize UI
    target_height: i32,
    /// Is left mouse button pressed (for dragging)
    is_left_pressed: bool,
    /// Is right mouse button pressed (for dragging)
    is_right_pressed: bool,
    /// Value to paint during drag (determined by first pixel toggled)
    draw_value: Option<bool>,
    /// Edit grid canvas cache
    pub(crate) edit_cache: iced::widget::canvas::Cache,
    /// Glyph selector canvas cache
    selector_cache: iced::widget::canvas::Cache,

    // ═══════════════════════════════════════════════════════════════════════
    // Tool & UI-only state
    // ═══════════════════════════════════════════════════════════════════════
    /// Current tool
    pub current_tool: BitFontTool,
    /// Drag start position for shapes/selection
    drag_start: Option<(i32, i32)>,
    /// Whether we're currently extending selection with shift
    is_selecting: bool,
    /// Tool panel
    tool_panel: BitFontToolPanel,
    /// Palette grid (in left sidebar)
    palette_grid: PaletteGrid,
    /// Top toolbar (color switcher + tool options)
    pub(crate) top_toolbar: BitFontTopToolbar,
    /// Whether preview mode is active
    pub show_preview: bool,
    /// Cached terminal screen for preview rendering
    preview_screen: Option<Arc<Mutex<Box<dyn icy_engine::Screen>>>>,
    /// Terminal instance used while preview is visible
    preview_terminal: Option<Terminal>,
    /// Monitor settings applied to preview terminal
    preview_monitor: Arc<MonitorSettings>,
}

impl BitFontEditor {
    /// Create a new BitFont editor with a default font
    pub fn new() -> Self {
        let state = BitFontEditState::new();
        let (width, height) = state.font_size();

        Self {
            state,
            target_width: width,
            target_height: height,
            is_left_pressed: false,
            is_right_pressed: false,
            draw_value: None,
            edit_cache: iced::widget::canvas::Cache::new(),
            selector_cache: iced::widget::canvas::Cache::new(),
            // New tool & cursor state
            current_tool: BitFontTool::Click,
            drag_start: None,
            is_selecting: false,
            tool_panel: BitFontToolPanel::new(),
            palette_grid: PaletteGrid::new(),
            top_toolbar: BitFontTopToolbar::new(),
            show_preview: false,
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: Arc::new(MonitorSettings::default()),
        }
    }

    /// Create a BitFont editor from a file
    pub fn from_file(path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();
        let font = BitFont::from_bytes(name, &data).map_err(|e| format!("Failed to parse font: {}", e))?;
        let size = font.size();
        let mut state = BitFontEditState::from_font(font.clone());
        state.set_file_path(Some(path.clone()));
        state.mark_clean();

        Ok(Self {
            target_width: size.width,
            target_height: size.height,
            is_left_pressed: false,
            is_right_pressed: false,
            draw_value: None,
            edit_cache: iced::widget::canvas::Cache::new(),
            selector_cache: iced::widget::canvas::Cache::new(),
            // New tool & cursor state
            current_tool: BitFontTool::Click,
            drag_start: None,
            is_selecting: false,
            tool_panel: BitFontToolPanel::new(),
            palette_grid: PaletteGrid::new(),
            top_toolbar: BitFontTopToolbar::new(),
            show_preview: false,
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: Arc::new(MonitorSettings::default()),
            state,
        })
    }

    /// Get the pixel data for a character
    pub fn get_glyph_pixels(&self, ch: char) -> &Vec<Vec<bool>> {
        self.state.get_glyph_pixels(ch)
    }

    pub(crate) fn selected_char(&self) -> char {
        self.state.selected_char()
    }

    fn set_selected_char(&mut self, ch: char) {
        self.state.set_selected_char(ch);
        self.edit_cache.clear();
        self.selector_cache.clear();
    }

    /// Resize the font to new dimensions
    pub fn resize_font(&mut self, width: i32, height: i32) {
        if self.state.resize_font(width, height).is_ok() {
            self.target_width = width;
            self.target_height = height;
            self.refresh_targets();
        }
    }

    pub(crate) fn cursor_pos(&self) -> (i32, i32) {
        self.state.cursor_pos()
    }

    /// Get drag start position for shape preview
    pub(crate) fn drag_start(&self) -> Option<(i32, i32)> {
        self.drag_start
    }

    /// Check if left mouse is pressed (for preview rendering)
    pub(crate) fn is_dragging(&self) -> bool {
        self.is_left_pressed && self.drag_start.is_some()
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        self.state.set_cursor_pos(x, y);
    }

    pub(crate) fn selection(&self) -> Option<(i32, i32, i32, i32)> {
        self.state.selection()
    }

    fn set_selection(&mut self, selection: Option<(i32, i32, i32, i32)>) {
        self.state.set_selection(selection);
    }

    fn clear_selection(&mut self) {
        self.state.clear_selection();
    }

    /// Get charset selection (anchor, lead, is_rectangle)
    pub(crate) fn charset_selection(&self) -> Option<(icy_engine::Position, icy_engine::Position, bool)> {
        self.state.charset_selection()
    }

    pub(crate) fn charset_cursor(&self) -> (i32, i32) {
        self.state.charset_cursor()
    }

    fn set_charset_cursor(&mut self, x: i32, y: i32) {
        self.state.set_charset_cursor(x, y);
    }

    pub(crate) fn use_letter_spacing(&self) -> bool {
        self.state.use_letter_spacing()
    }

    pub fn invalidate_caches(&mut self) {
        self.edit_cache.clear();
        self.selector_cache.clear();
    }

    /// Calculate scale factors for edit grid and charset based on available height
    ///
    /// Returns (edit_cell_scale, charset_scale) where:
    /// - edit_cell_scale: multiplier for EDIT_CELL_SIZE (1.0 = default 30px cells)
    /// - charset_scale: multiplier for font size in charset (2.0 = default)
    fn calculate_grid_scales(&self) -> (f32, f32) {
        let (_font_width, font_height) = self.state.font_size();

        // Default scales
        let default_edit_scale = 1.0;
        let default_charset_scale = 2.0;

        // Calculate ideal heights at default scales
        // Edit grid: RULER_SIZE + (CELL_SIZE + CELL_GAP) * font_height
        let ideal_edit_height = RULER_SIZE + (EDIT_CELL_SIZE + EDIT_CELL_BORDER) * font_height as f32;

        // Charset: RULER_SIZE + 16 rows * (font_height * scale)
        let ideal_charset_height = RULER_SIZE + 16.0 * (font_height as f32 * default_charset_scale);

        // Use the taller one to determine if scaling is needed
        let max_ideal_height = ideal_edit_height.max(ideal_charset_height);

        if max_ideal_height <= MAX_GRID_HEIGHT {
            // Everything fits at default scale
            (default_edit_scale, default_charset_scale)
        } else {
            // Need to scale down proportionally
            // Calculate the scale factor needed to fit the taller grid
            let scale_factor = ((MAX_GRID_HEIGHT - RULER_SIZE) / (max_ideal_height - RULER_SIZE)).max(MIN_SCALE_FACTOR);

            let edit_scale = default_edit_scale * scale_factor;
            let charset_scale = default_charset_scale * scale_factor;

            (edit_scale, charset_scale)
        }
    }

    /// Get the current edit cell size (scaled)
    pub(crate) fn scaled_edit_cell_size(&self) -> f32 {
        let (edit_scale, _) = self.calculate_grid_scales();
        EDIT_CELL_SIZE * edit_scale
    }

    /// Get the current edit cell gap (scaled)
    pub(crate) fn scaled_edit_cell_gap(&self) -> f32 {
        let (edit_scale, _) = self.calculate_grid_scales();
        EDIT_CELL_BORDER * edit_scale
    }

    fn refresh_targets(&mut self) {
        let (width, height) = self.state.font_size();
        self.target_width = width;
        self.target_height = height;
    }

    fn rebuild_preview_terminal(&mut self) {
        let fg = self.top_toolbar.foreground as u8;
        let bg = self.top_toolbar.background as u8;
        let screen = self.state.build_preview_content_for(self.selected_char(), fg, bg);
        let boxed: Box<dyn icy_engine::Screen> = Box::new(screen);

        if let Some(screen_arc) = &self.preview_screen {
            let mut guard = screen_arc.lock();
            *guard = boxed;
        } else {
            let screen_arc: Arc<Mutex<Box<dyn icy_engine::Screen>>> = Arc::new(Mutex::new(boxed));
            let mut terminal = Terminal::new(screen_arc.clone());
            terminal.set_fit_terminal_height_to_bounds(true);
            terminal.update_viewport_size();
            self.preview_screen = Some(screen_arc);
            self.preview_terminal = Some(terminal);
        }

        if let Some(terminal) = self.preview_terminal.as_mut() {
            if let Some(screen_arc) = &self.preview_screen {
                terminal.screen = screen_arc.clone();
            }
            terminal.update_viewport_size();
        }
    }

    /// Set a single pixel (with flood prevention - does nothing if already set to value)
    fn set_pixel(&mut self, x: i32, y: i32, value: bool) {
        let ch = self.selected_char();
        // Check if pixel already has the desired value
        let pixels = self.state.get_glyph_pixels(ch);
        let current = pixels.get(y as usize).and_then(|row| row.get(x as usize)).copied().unwrap_or(false);
        if current == value {
            return; // No change needed
        }
        let _ = self.state.set_pixel(ch, x, y, value);
        self.invalidate_caches();
    }

    /// Get font size
    pub fn font_size(&self) -> (i32, i32) {
        self.state.font_size()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // UndoHandler-like interface
    // ═══════════════════════════════════════════════════════════════════════

    /// Get description of next undo operation
    pub fn undo_description(&self) -> Option<String> {
        self.state.undo_description()
    }

    /// Check if undo is available
    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        self.state.can_undo()
    }

    /// Perform undo
    pub fn undo(&mut self) {
        if self.state.undo().is_ok() {
            self.refresh_targets();
            self.invalidate_caches();
        }
    }

    /// Get description of next redo operation
    pub fn redo_description(&self) -> Option<String> {
        self.state.redo_description()
    }

    /// Check if redo is available
    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        self.state.can_redo()
    }

    /// Perform redo
    pub fn redo(&mut self) {
        if self.state.redo().is_ok() {
            self.refresh_targets();
            self.invalidate_caches();
        }
    }

    /// Get undo stack length (for modified tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.state.undo_stack_len()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // MCP API Methods
    // ═══════════════════════════════════════════════════════════════════════

    /// Get list of all available character codes in the font
    pub fn list_char_codes(&self) -> Vec<u32> {
        // BitFonts support characters 0-255
        (0u32..256).collect()
    }

    /// Get glyph data for a specific character code
    ///
    /// Returns GlyphData with base64-encoded bitmap (row-major, MSB first)
    pub fn get_glyph_data(&self, code: u32) -> Result<crate::mcp::types::GlyphData, String> {
        if code > 255 {
            return Err(format!("Character code {} out of range (0-255)", code));
        }
        let ch = char::from_u32(code).ok_or_else(|| format!("Invalid character code: {}", code))?;
        let pixels = self.state.get_glyph_pixels(ch);
        let (width, height) = self.font_size();

        // Convert bool grid to packed bits (row-major, MSB first)
        let bytes_per_row = (width as usize + 7) / 8;
        let mut bitmap = vec![0u8; bytes_per_row * height as usize];

        for (y, row) in pixels.iter().enumerate() {
            if y >= height as usize {
                break;
            }
            for (x, &pixel) in row.iter().enumerate() {
                if x >= width as usize {
                    break;
                }
                if pixel {
                    let byte_idx = y * bytes_per_row + x / 8;
                    let bit_idx = 7 - (x % 8); // MSB first
                    bitmap[byte_idx] |= 1 << bit_idx;
                }
            }
        }

        use base64::prelude::*;
        let encoded = BASE64_STANDARD.encode(&bitmap);

        // Get printable character representation if available
        let char_str = if ch.is_ascii_graphic() || ch == ' ' { Some(ch.to_string()) } else { None };

        Ok(crate::mcp::types::GlyphData {
            code,
            char: char_str,
            width,
            height,
            bitmap: encoded,
        })
    }

    /// Set glyph data for a specific character code
    ///
    /// Takes GlyphData with base64-encoded bitmap in row-major format, MSB first
    pub fn set_glyph_data(&mut self, data: &crate::mcp::types::GlyphData) -> Result<(), String> {
        if data.code > 255 {
            return Err(format!("Character code {} out of range (0-255)", data.code));
        }
        let ch = char::from_u32(data.code).ok_or_else(|| format!("Invalid character code: {}", data.code))?;

        use base64::prelude::*;
        let bitmap = BASE64_STANDARD.decode(&data.bitmap).map_err(|e| format!("Invalid base64: {}", e))?;

        let (font_width, font_height) = self.font_size();
        if data.width != font_width || data.height != font_height {
            return Err(format!(
                "Glyph size {}x{} doesn't match font size {}x{}",
                data.width, data.height, font_width, font_height
            ));
        }

        let bytes_per_row = (data.width as usize + 7) / 8;
        let expected_size = bytes_per_row * data.height as usize;
        if bitmap.len() != expected_size {
            return Err(format!(
                "Bitmap size {} doesn't match expected {} ({}x{} at {} bytes/row)",
                bitmap.len(),
                expected_size,
                data.width,
                data.height,
                bytes_per_row
            ));
        }

        // Convert packed bits to bool grid
        let mut pixels = vec![vec![false; data.width as usize]; data.height as usize];
        for y in 0..data.height as usize {
            for x in 0..data.width as usize {
                let byte_idx = y * bytes_per_row + x / 8;
                let bit_idx = 7 - (x % 8); // MSB first
                pixels[y][x] = (bitmap[byte_idx] >> bit_idx) & 1 != 0;
            }
        }

        // Use set_glyph_pixels which handles undo properly
        self.state.set_glyph_pixels(ch, pixels).map_err(|e| format!("Failed to set glyph: {}", e))?;

        self.invalidate_caches();
        Ok(())
    }

    /// Get number of glyphs in the font (always 256 for BitFont)
    pub fn glyph_count(&self) -> usize {
        256
    }

    /// Get first character code (always 0 for BitFont)
    pub fn first_char(&self) -> u32 {
        0
    }

    /// Get last character code (always 255 for BitFont)
    pub fn last_char(&self) -> u32 {
        255
    }

    /// Get currently selected character code as u32 (for MCP API)
    pub fn selected_char_code(&self) -> u32 {
        self.state.selected_char() as u32
    }

    /// Get session data for serialization
    pub fn get_session_data(&self) -> Option<icy_engine_edit::bitfont::BitFontSessionState> {
        Some(icy_engine_edit::bitfont::BitFontSessionState {
            version: 1,
            undo_stack: self.state.undo_stack().clone(),
            selected_glyph: self.state.selected_char() as usize,
            cursor_position: self.state.cursor_pos(),
            edit_zoom: 1.0, // TODO: store actual zoom
            selector_zoom: 1.0,
            focused_panel: match self.state.focused_panel() {
                icy_engine_edit::bitfont::BitFontFocusedPanel::EditGrid => icy_engine_edit::bitfont::BitFontFocusedPanelState::EditGrid,
                icy_engine_edit::bitfont::BitFontFocusedPanel::CharSet => icy_engine_edit::bitfont::BitFontFocusedPanelState::CharSet,
            },
            show_grid: true,
            selected_tool: String::new(),
        })
    }

    /// Restore session data from serialization
    pub fn set_session_data(&mut self, state: icy_engine_edit::bitfont::BitFontSessionState) {
        *self.state.undo_stack_mut() = state.undo_stack;
        self.state.set_selected_char(char::from_u32(state.selected_glyph as u32).unwrap_or('\0'));
        self.state.set_cursor_pos(state.cursor_position.0, state.cursor_position.1);
        let panel = match state.focused_panel {
            icy_engine_edit::bitfont::BitFontFocusedPanelState::EditGrid => icy_engine_edit::bitfont::BitFontFocusedPanel::EditGrid,
            icy_engine_edit::bitfont::BitFontFocusedPanelState::CharSet => icy_engine_edit::bitfont::BitFontFocusedPanel::CharSet,
        };
        self.state.set_focused_panel(panel);
        self.invalidate_caches();
    }

    /// Get bytes for autosave (saves in PSF2 format)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        let font = self.state.build_font();
        font.to_psf2_bytes().map_err(|e| e.to_string())
    }

    /// Load from an autosave file, using the original path for file association
    ///
    /// The autosave file is always saved as PSF2 format.
    pub fn load_from_autosave(autosave_path: &std::path::Path, original_path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read(autosave_path).map_err(|e| format!("Failed to read autosave: {}", e))?;
        let name = original_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Font").to_string();
        let font = BitFont::from_bytes(name, &data).map_err(|e| format!("Failed to parse font: {}", e))?;
        let size = font.size();
        let mut state = BitFontEditState::from_font(font.clone());
        state.set_file_path(Some(original_path));
        // Don't mark clean - this is an autosave recovery

        Ok(Self {
            target_width: size.width,
            target_height: size.height,
            is_left_pressed: false,
            is_right_pressed: false,
            draw_value: None,
            edit_cache: iced::widget::canvas::Cache::new(),
            selector_cache: iced::widget::canvas::Cache::new(),
            current_tool: BitFontTool::Click,
            drag_start: None,
            is_selecting: false,
            tool_panel: BitFontToolPanel::new(),
            palette_grid: PaletteGrid::new(),
            top_toolbar: BitFontTopToolbar::new(),
            show_preview: false,
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: Arc::new(MonitorSettings::default()),
            state,
        })
    }

    /// Handle update messages
    pub fn update(&mut self, message: BitFontEditorMessage, dialogs: &mut DialogStack<Message>) -> Task<BitFontEditorMessage> {
        match message {
            // ═══════════════════════════════════════════════════════════════
            // Dialog-related messages (moved from MainWindow)
            // ═══════════════════════════════════════════════════════════════
            BitFontEditorMessage::ShowFontSizeDialog => {
                let (width, height) = self.font_size();
                dialogs.push(FontSizeDialog::new(width, height));
                return Task::none();
            }
            BitFontEditorMessage::FontSizeDialog(_) => {
                // Handled by DialogStack
                return Task::none();
            }
            BitFontEditorMessage::FontSizeApply(width, height) => {
                let _ = self.resize_font(width, height);
                self.invalidate_caches();
                return Task::none();
            }
            BitFontEditorMessage::FontImportDialog(_) => {
                // Handled by DialogStack
                return Task::none();
            }
            BitFontEditorMessage::FontImported(_) => {
                // This is handled specially by MainWindow to switch mode
                return Task::none();
            }
            BitFontEditorMessage::ShowExportFontDialog => {
                let font = self.state.build_font();
                dialogs.push(crate::ui::dialog::font_export::FontExportDialog::new(font));
                return Task::none();
            }
            BitFontEditorMessage::FontExportDialog(_) => {
                // Handled by DialogStack
                return Task::none();
            }
            BitFontEditorMessage::FontExported => {
                return Task::none();
            }

            // ═══════════════════════════════════════════════════════════════
            // Character selection
            // ═══════════════════════════════════════════════════════════════
            BitFontEditorMessage::SelectGlyph(ch) => {
                self.set_selected_char(ch);
                self.edit_cache.clear();
            }
            BitFontEditorMessage::SelectGlyphAt(ch, col, row) => {
                self.set_selected_char(ch);
                self.set_charset_cursor(col.clamp(0, 15), row.clamp(0, 15));
                // Clear selection on click - selection only starts on drag
                self.state.clear_charset_selection();
                self.state.set_focused_panel(BitFontFocusedPanel::CharSet);
                self.edit_cache.clear();
                self.selector_cache.clear();
            }
            BitFontEditorMessage::SetPixel(x, y, value) => {
                self.set_pixel(x, y, value);
            }
            BitFontEditorMessage::Clear => {
                self.erase_selection();
            }
            BitFontEditorMessage::Inverse => {
                self.inverse_selection();
            }
            BitFontEditorMessage::MoveUp => {
                let _ = self.state.move_glyph(self.selected_char(), 0, -1);
                self.invalidate_caches();
            }
            BitFontEditorMessage::MoveDown => {
                let _ = self.state.move_glyph(self.selected_char(), 0, 1);
                self.invalidate_caches();
            }
            BitFontEditorMessage::MoveLeft => {
                let _ = self.state.move_glyph(self.selected_char(), -1, 0);
                self.invalidate_caches();
            }
            BitFontEditorMessage::MoveRight => {
                let _ = self.state.move_glyph(self.selected_char(), 1, 0);
                self.invalidate_caches();
            }
            BitFontEditorMessage::FlipX => {
                let _ = self.state.flip_glyph_x(self.selected_char());
                self.invalidate_caches();
            }
            BitFontEditorMessage::FlipY => {
                let _ = self.state.flip_glyph_y(self.selected_char());
                self.invalidate_caches();
            }
            BitFontEditorMessage::SlideUp => {
                let _ = self.state.slide_glyph(0, -1);
                self.invalidate_caches();
            }
            BitFontEditorMessage::SlideDown => {
                let _ = self.state.slide_glyph(0, 1);
                self.invalidate_caches();
            }
            BitFontEditorMessage::SlideLeft => {
                let _ = self.state.slide_glyph(-1, 0);
                self.invalidate_caches();
            }
            BitFontEditorMessage::SlideRight => {
                let _ = self.state.slide_glyph(1, 0);
                self.invalidate_caches();
            }
            BitFontEditorMessage::SetWidth(w) => {
                self.target_width = w.clamp(1, 16);
            }
            BitFontEditorMessage::SetHeight(h) => {
                self.target_height = h.clamp(1, 32);
            }
            BitFontEditorMessage::ApplyResize => {
                let (width, height) = self.state.font_size();
                if self.target_width != width || self.target_height != height {
                    let _ = self.state.resize_font(self.target_width, self.target_height);
                    self.refresh_targets();
                    self.invalidate_caches();
                }
            }
            BitFontEditorMessage::Undo => {
                let _ = self.state.undo();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::Redo => {
                let _ = self.state.redo();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::CanvasEvent(event) => {
                self.handle_canvas_event(event);
            }

            // ═══════════════════════════════════════════════════════════════
            // Tool & Cursor handling
            // ═══════════════════════════════════════════════════════════════
            BitFontEditorMessage::SelectTool(tool) => {
                self.current_tool = tool;
                self.clear_selection();
                self.is_selecting = false;
                self.edit_cache.clear();
            }
            BitFontEditorMessage::ToggleRectFilled => {
                self.current_tool = match self.current_tool {
                    BitFontTool::RectangleOutline => BitFontTool::RectangleFilled,
                    BitFontTool::RectangleFilled => BitFontTool::RectangleOutline,
                    other => other,
                };
            }
            BitFontEditorMessage::MoveCursor(dx, dy) => {
                self.move_cursor(dx, dy);
            }
            BitFontEditorMessage::TogglePixelAtCursor => {
                self.toggle_pixel_at_cursor();
            }
            BitFontEditorMessage::SetPixelAtCursor(value) => {
                self.set_pixel_at_cursor(value);
            }
            BitFontEditorMessage::ExtendSelection(dx, dy) => {
                self.extend_selection(dx, dy);
            }
            BitFontEditorMessage::ExtendCharsetSelection(dx, dy, is_rectangle) => {
                self.extend_charset_selection(dx, dy, is_rectangle);
            }
            BitFontEditorMessage::SetCharsetSelectionLead(col, row, is_rectangle) => {
                self.set_charset_selection_lead(col, row, is_rectangle);
            }
            BitFontEditorMessage::ClearSelection => {
                self.clear_selection();
                self.is_selecting = false;
                self.edit_cache.clear();
            }
            BitFontEditorMessage::ClearCharsetSelection => {
                self.state.clear_charset_selection();
                self.selector_cache.clear();
            }
            BitFontEditorMessage::SelectAll => {
                let (width, height) = self.state.font_size();
                self.set_selection(Some((0, 0, width - 1, height - 1)));
                self.edit_cache.clear();
            }
            BitFontEditorMessage::FillSelection => {
                self.fill_selection();
            }
            BitFontEditorMessage::EraseSelection => {
                self.erase_selection();
            }
            BitFontEditorMessage::InverseSelection => {
                self.inverse_selection();
            }
            BitFontEditorMessage::NextChar => {
                let current = self.selected_char();
                let next = ((current as u32) + 1).min(255);
                let new_char = char::from_u32(next).unwrap_or(current);
                self.set_selected_char(new_char);
            }
            BitFontEditorMessage::PrevChar => {
                let current = self.selected_char();
                let prev = (current as u32).saturating_sub(1);
                let new_char = char::from_u32(prev).unwrap_or(current);
                self.set_selected_char(new_char);
            }
            BitFontEditorMessage::ToggleLetterSpacing => {
                self.state.toggle_letter_spacing();
                self.invalidate_caches();
            }
            BitFontEditorMessage::InsertLine => {
                let _ = self.state.insert_line();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::DeleteLine => {
                let _ = self.state.delete_line();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::InsertColumn => {
                let _ = self.state.insert_column();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::DeleteColumn => {
                let _ = self.state.delete_column();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::DuplicateLine => {
                let _ = self.state.duplicate_line();
                self.refresh_targets();
                self.invalidate_caches();
            }
            BitFontEditorMessage::SwapChars => {
                // Swap selected char with char at charset cursor
                let (cx, cy) = self.charset_cursor();
                let cursor_char_code = (cy * 16 + cx) as u32;
                if let Some(cursor_char) = char::from_u32(cursor_char_code) {
                    let selected = self.selected_char();
                    if cursor_char != selected {
                        let _ = self.state.swap_chars(selected, cursor_char);
                        self.invalidate_caches();
                    }
                }
            }
            BitFontEditorMessage::FocusNextPanel => {
                let new_panel = match self.state.focused_panel() {
                    BitFontFocusedPanel::EditGrid => BitFontFocusedPanel::CharSet,
                    BitFontFocusedPanel::CharSet => BitFontFocusedPanel::EditGrid,
                };
                self.state.set_focused_panel(new_panel);
                self.edit_cache.clear();
                self.selector_cache.clear();
            }
            BitFontEditorMessage::SetFocusedPanel(panel) => {
                self.state.set_focused_panel(panel);
                self.edit_cache.clear();
                self.selector_cache.clear();
            }
            BitFontEditorMessage::MoveCharsetCursor(dx, dy) => {
                self.state.move_charset_cursor(dx, dy);
                self.selector_cache.clear();
            }
            BitFontEditorMessage::SetCharsetCursor(x, y) => {
                self.set_charset_cursor(x.clamp(0, 15), y.clamp(0, 15));
                self.selector_cache.clear();
            }
            BitFontEditorMessage::SelectCharAtCursor => {
                let (x, y) = self.charset_cursor();
                let ch_code = (y * 16 + x) as u32;
                if let Some(ch) = char::from_u32(ch_code) {
                    self.set_selected_char(ch);
                    self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
                    self.edit_cache.clear();
                    self.selector_cache.clear();
                }
            }
            BitFontEditorMessage::ToolPanel(msg) => match msg {
                BitFontToolPanelMessage::ClickSlot(slot) => {
                    let tool = self.tool_panel.click_slot(slot);
                    self.current_tool = tool;
                }
                BitFontToolPanelMessage::Tick(delta) => {
                    self.tool_panel.tick(delta);
                }
            },
            BitFontEditorMessage::PaletteGrid(msg) => match msg {
                PaletteGridMessage::SetForeground(color) => {
                    self.top_toolbar.foreground = color;
                    self.palette_grid.set_foreground(color);
                    self.edit_cache.clear();
                    self.selector_cache.clear();
                }
                PaletteGridMessage::SetBackground(color) => {
                    self.top_toolbar.background = color;
                    self.palette_grid.set_background(color);
                    self.edit_cache.clear();
                    self.selector_cache.clear();
                }
            },
            BitFontEditorMessage::TopToolbar(msg) => {
                // Handle direct navigation first
                match msg {
                    BitFontTopToolbarMessage::NextChar => {
                        // NextChar logic inline
                        let current = self.selected_char();
                        let next = ((current as u32) + 1).min(255);
                        let new_char = char::from_u32(next).unwrap_or(current);
                        self.set_selected_char(new_char);
                        self.edit_cache.clear();
                        return Task::none();
                    }
                    BitFontTopToolbarMessage::PrevChar => {
                        // PrevChar logic inline
                        let current = self.selected_char();
                        let prev = (current as u32).saturating_sub(1);
                        let new_char = char::from_u32(prev).unwrap_or(current);
                        self.set_selected_char(new_char);
                        self.edit_cache.clear();
                        return Task::none();
                    }
                    _ => {}
                }

                let old_fg = self.top_toolbar.foreground;
                let old_bg = self.top_toolbar.background;

                let task = self.top_toolbar.update(msg);

                if self.top_toolbar.foreground != old_fg {
                    self.palette_grid.set_foreground(self.top_toolbar.foreground);
                }
                if self.top_toolbar.background != old_bg {
                    self.palette_grid.set_background(self.top_toolbar.background);
                }

                if self.top_toolbar.foreground != old_fg || self.top_toolbar.background != old_bg {
                    self.edit_cache.clear();
                    self.selector_cache.clear();
                }

                return task.map(BitFontEditorMessage::TopToolbar);
            }
            BitFontEditorMessage::ShowPreview => {
                self.rebuild_preview_terminal();
                self.show_preview = true;
            }
            BitFontEditorMessage::HidePreview => {
                self.show_preview = false;
            }
            BitFontEditorMessage::PreviewTerminal(_msg) => {}

            // ═══════════════════════════════════════════════════════════════
            // Generic keyboard event handling (panel-agnostic)
            // ═══════════════════════════════════════════════════════════════
            BitFontEditorMessage::HandleArrow(direction, modifiers) => {
                return self.handle_arrow_key(direction, modifiers);
            }
            BitFontEditorMessage::HandleConfirm => {
                return self.handle_confirm();
            }
            BitFontEditorMessage::HandleCancel => {
                return self.handle_cancel();
            }
            BitFontEditorMessage::HandleHome => {
                return self.handle_home();
            }
            BitFontEditorMessage::HandleEnd => {
                return self.handle_end();
            }
            BitFontEditorMessage::HandlePageUp => {
                return self.handle_page_up();
            }
            BitFontEditorMessage::HandlePageDown => {
                return self.handle_page_down();
            }
        }
        Task::none()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Cursor & Selection helpers
    // ═══════════════════════════════════════════════════════════════════════

    /// Move cursor by delta, clamping to bounds
    /// Clears any existing selection (like text editors)
    fn move_cursor(&mut self, dx: i32, dy: i32) {
        // Clear selection when moving cursor without modifiers
        if self.selection().is_some() {
            self.clear_selection();
            self.is_selecting = false;
        }
        self.state.move_cursor(dx, dy);
        self.edit_cache.clear();
    }

    /// Toggle pixel at current cursor position
    fn toggle_pixel_at_cursor(&mut self) {
        let (x, y) = self.cursor_pos();
        let _ = self.state.toggle_pixel(self.selected_char(), x, y);
        self.invalidate_caches();
    }

    /// Set pixel at cursor to specific value
    fn set_pixel_at_cursor(&mut self, value: bool) {
        let (x, y) = self.cursor_pos();
        self.set_pixel(x, y, value);
    }

    /// Extend selection from cursor with shift+arrows
    fn extend_selection(&mut self, dx: i32, dy: i32) {
        if !self.is_selecting {
            // Start new selection from cursor using Selection struct
            self.is_selecting = true;
            self.state.start_edit_selection();
        }

        // Move cursor AND extend selection in one undo operation
        self.state.move_cursor_and_extend_selection(dx, dy);
        self.edit_cache.clear();
    }

    /// Extend charset selection with shift+arrows (anchor/lead mode)
    /// Anchor stays at original position, lead moves with cursor
    /// is_rectangle: true = Alt held (rectangle mode), false = linear (default)
    fn extend_charset_selection(&mut self, dx: i32, dy: i32, is_rectangle: bool) {
        // move_charset_cursor_and_extend_selection handles starting a new selection
        // if none exists, so we don't need to call start_charset_selection_with_mode
        self.state.move_charset_cursor_and_extend_selection(dx, dy, is_rectangle);
        self.selector_cache.clear();
    }

    /// Set charset selection lead position directly (for mouse drag)
    /// Anchor stays fixed where selection started, lead follows mouse
    /// is_rectangle: true = Alt held (rectangle mode), false = linear (default)
    fn set_charset_selection_lead(&mut self, col: i32, row: i32, is_rectangle: bool) {
        if self.state.charset_selection().is_none() {
            // Start new charset selection at current cursor (becomes anchor)
            self.state.start_charset_selection_with_mode(is_rectangle);
        }

        // Set cursor to new position (becomes lead)
        self.state.set_charset_cursor(col, row);

        // Extend selection - updates lead to current cursor position
        self.state.extend_charset_selection_with_mode(is_rectangle);
        self.selector_cache.clear();
    }

    /// Fill selection with pixels (set to true)
    fn fill_selection(&mut self) {
        let _ = self.state.fill_selection();
        self.invalidate_caches();
    }

    /// Erase selection (set pixels to false)
    fn erase_selection(&mut self) {
        let _ = self.state.erase_selection();
        self.invalidate_caches();
    }

    /// Inverse pixels in selection (or whole glyph)
    fn inverse_selection(&mut self) {
        let _ = self.state.inverse_edit_selection();
        self.invalidate_caches();
    }

    /// Handle canvas interaction events
    fn handle_canvas_event(&mut self, event: CanvasEvent) {
        match event {
            CanvasEvent::LeftPressed(pos) => {
                self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
                self.is_left_pressed = true;

                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    match self.current_tool {
                        BitFontTool::Select => {
                            // Start selection at clicked position
                            self.state.set_cursor_pos(x, y);
                            self.state.start_edit_selection();
                            self.is_selecting = true;
                        }
                        BitFontTool::Click => {
                            // If there's a selection, clear it on click
                            if self.selection().is_some() {
                                self.clear_selection();
                                self.is_selecting = false;
                            } else {
                                // Toggle pixel and store the new value for drag painting
                                let ch = self.selected_char();
                                let pixels = self.state.get_glyph_pixels(ch);
                                let current = pixels.get(y as usize).and_then(|row| row.get(x as usize)).copied().unwrap_or(false);
                                let new_value = !current;
                                self.draw_value = Some(new_value);
                                self.set_pixel(x, y, new_value);
                            }
                        }
                        BitFontTool::Fill => {
                            // Perform flood fill immediately on click
                            let ch = self.selected_char();
                            let _ = self.state.flood_fill(ch, x, y, true);
                            self.invalidate_caches();
                        }
                        BitFontTool::Line | BitFontTool::RectangleOutline | BitFontTool::RectangleFilled => {
                            // Start shape drag - set both start position AND cursor
                            self.drag_start = Some((x, y));
                            self.set_cursor_pos(x, y);
                        }
                    }
                }
                self.edit_cache.clear();
                self.selector_cache.clear();
            }
            CanvasEvent::RightPressed(pos) => {
                self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
                self.is_right_pressed = true;
                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    match self.current_tool {
                        BitFontTool::Fill => {
                            // Right-click flood fill erases (fills with false)
                            let ch = self.selected_char();
                            let _ = self.state.flood_fill(ch, x, y, false);
                            self.invalidate_caches();
                        }
                        _ => {
                            self.set_pixel(x, y, false);
                        }
                    }
                }
                self.selector_cache.clear();
            }
            CanvasEvent::LeftReleased => {
                if self.is_left_pressed {
                    self.is_left_pressed = false;

                    // Commit shape tools on release
                    if let Some((start_x, start_y)) = self.drag_start.take() {
                        let (end_x, end_y) = self.cursor_pos();
                        let ch = self.selected_char();
                        match self.current_tool {
                            BitFontTool::Line => {
                                let _ = self.state.draw_line(ch, start_x, start_y, end_x, end_y, true);
                            }
                            BitFontTool::RectangleOutline => {
                                let _ = self.state.draw_rectangle(ch, start_x, start_y, end_x, end_y, false, true);
                            }
                            BitFontTool::RectangleFilled => {
                                let _ = self.state.draw_rectangle(ch, start_x, start_y, end_x, end_y, true, true);
                            }
                            _ => {}
                        }
                        self.edit_cache.clear();
                    }

                    self.draw_value = None;
                }
            }
            CanvasEvent::RightReleased => {
                if self.is_right_pressed {
                    self.is_right_pressed = false;
                }
            }
            CanvasEvent::MiddlePressed => {
                // Toggle between Select tool and Click tool
                if self.current_tool == BitFontTool::Select {
                    self.current_tool = BitFontTool::Click;
                } else {
                    self.current_tool = BitFontTool::Select;
                }
                // Update tool panel to reflect the change
                self.tool_panel.set_tool(self.current_tool);
            }
            CanvasEvent::CursorMoved(pos) => {
                if let Some((x, y)) = self.pos_to_pixel(pos) {
                    // Update cursor position for shape preview
                    if self.is_left_pressed && self.drag_start.is_some() {
                        self.set_cursor_pos(x, y);
                        self.edit_cache.clear();
                    } else if self.is_left_pressed {
                        match self.current_tool {
                            BitFontTool::Select => {
                                // Extend selection to current position
                                self.state.set_cursor_pos(x, y);
                                self.state.extend_edit_selection();
                                self.edit_cache.clear();
                            }
                            BitFontTool::Click => {
                                // Continue painting with the same value as the initial toggle
                                if let Some(value) = self.draw_value {
                                    self.set_pixel(x, y, value);
                                }
                            }
                            _ => {}
                        }
                    } else if self.is_right_pressed {
                        self.set_pixel(x, y, false);
                    }
                }
            }
        }
    }

    /// Convert canvas position to pixel coordinates
    fn pos_to_pixel(&self, pos: Point) -> Option<(i32, i32)> {
        let scaled_cell_size = self.scaled_edit_cell_size();
        let scaled_cell_gap = self.scaled_edit_cell_gap();
        let x = ((pos.x - RULER_SIZE) / (scaled_cell_size + scaled_cell_gap)) as i32;
        let y = ((pos.y - RULER_SIZE) / (scaled_cell_size + scaled_cell_gap)) as i32;

        let (width, height) = self.state.font_size();
        if x >= 0 && x < width && y >= 0 && y < height {
            Some((x, y))
        } else {
            None
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Generic keyboard event handlers (panel-aware)
    // ═══════════════════════════════════════════════════════════════════════

    /// Handle arrow key based on focused panel and modifiers
    fn handle_arrow_key(&mut self, direction: ArrowDirection, modifiers: iced::keyboard::Modifiers) -> Task<BitFontEditorMessage> {
        let (dx, dy) = match direction {
            ArrowDirection::Up => (0, -1),
            ArrowDirection::Down => (0, 1),
            ArrowDirection::Left => (-1, 0),
            ArrowDirection::Right => (1, 0),
        };

        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                if modifiers.control() {
                    // Ctrl+Arrow: Slide pixels (rotate)
                    let _ = self.state.slide_glyph(dx, dy);
                    self.invalidate_caches();
                } else if modifiers.alt() {
                    // Alt+Arrow: Insert/Delete line/column
                    match direction {
                        ArrowDirection::Up => {
                            let _ = self.state.delete_line();
                            self.refresh_targets();
                            self.invalidate_caches();
                        }
                        ArrowDirection::Down => {
                            let _ = self.state.insert_line();
                            self.refresh_targets();
                            self.invalidate_caches();
                        }
                        ArrowDirection::Left => {
                            let _ = self.state.delete_column();
                            self.refresh_targets();
                            self.invalidate_caches();
                        }
                        ArrowDirection::Right => {
                            let _ = self.state.insert_column();
                            self.refresh_targets();
                            self.invalidate_caches();
                        }
                    }
                } else if modifiers.shift() {
                    // Shift+Arrow: Extend selection
                    self.extend_selection(dx, dy);
                } else {
                    // Arrow only: Move cursor
                    self.move_cursor(dx, dy);
                }
            }
            BitFontFocusedPanel::CharSet => {
                if modifiers.control() {
                    // Ctrl+Arrow: Slide pixels for all selected chars
                    let _ = self.state.slide_glyph(dx, dy);
                    self.invalidate_caches();
                } else if modifiers.shift() {
                    // Shift+Arrow: Extend charset selection
                    // Alt+Shift = rectangle mode, Shift only = linear mode
                    self.extend_charset_selection(dx, dy, modifiers.alt());
                } else {
                    // Arrow only: Move charset cursor (clears selection)
                    if self.charset_selection().is_some() {
                        self.state.clear_charset_selection();
                    }
                    self.state.move_charset_cursor(dx, dy);
                    self.selector_cache.clear();
                }
            }
        }
        Task::none()
    }

    /// Handle confirm action (Space/Enter) based on focused panel
    fn handle_confirm(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Toggle pixel at cursor
                self.toggle_pixel_at_cursor();
            }
            BitFontFocusedPanel::CharSet => {
                // Select character at cursor and switch to edit
                let (x, y) = self.charset_cursor();
                let ch_code = (y * 16 + x) as u32;
                if let Some(ch) = char::from_u32(ch_code) {
                    self.set_selected_char(ch);
                    self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
                    self.selector_cache.clear();
                }
            }
        }
        Task::none()
    }

    /// Handle cancel action (Escape) based on focused panel
    fn handle_cancel(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Clear edit selection
                self.clear_selection();
                self.is_selecting = false;
                self.edit_cache.clear();
            }
            BitFontFocusedPanel::CharSet => {
                // Clear charset selection
                self.state.clear_charset_selection();
                self.selector_cache.clear();
            }
        }
        Task::none()
    }

    /// Handle Home key - go to beginning of current line
    fn handle_home(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Move to column 0
                if self.selection().is_some() {
                    self.clear_selection();
                    self.is_selecting = false;
                }
                let (_, y) = self.cursor_pos();
                self.set_cursor_pos(0, y);
                self.edit_cache.clear();
            }
            BitFontFocusedPanel::CharSet => {
                // Move to column 0
                self.state.clear_charset_selection();
                let (_, y) = self.charset_cursor();
                self.set_charset_cursor(0, y);
                self.selector_cache.clear();
            }
        }
        Task::none()
    }

    /// Handle End key - go to end of current line
    fn handle_end(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Move to last column
                if self.selection().is_some() {
                    self.clear_selection();
                    self.is_selecting = false;
                }
                let (width, _) = self.font_size();
                let (_, y) = self.cursor_pos();
                self.set_cursor_pos(width - 1, y);
                self.edit_cache.clear();
            }
            BitFontFocusedPanel::CharSet => {
                // Move to column 15 (last column in 16x16 grid)
                self.state.clear_charset_selection();
                let (_, y) = self.charset_cursor();
                self.set_charset_cursor(15, y);
                self.selector_cache.clear();
            }
        }
        Task::none()
    }

    /// Handle PageUp key - go to top row
    fn handle_page_up(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Move to row 0
                if self.selection().is_some() {
                    self.clear_selection();
                    self.is_selecting = false;
                }
                let (x, _) = self.cursor_pos();
                self.set_cursor_pos(x, 0);
                self.edit_cache.clear();
            }
            BitFontFocusedPanel::CharSet => {
                // Move to row 0
                self.state.clear_charset_selection();
                let (x, _) = self.charset_cursor();
                self.set_charset_cursor(x, 0);
                self.selector_cache.clear();
            }
        }
        Task::none()
    }

    /// Handle PageDown key - go to bottom row
    fn handle_page_down(&mut self) -> Task<BitFontEditorMessage> {
        match self.state.focused_panel() {
            BitFontFocusedPanel::EditGrid => {
                // Move to last row
                if self.selection().is_some() {
                    self.clear_selection();
                    self.is_selecting = false;
                }
                let (_, height) = self.font_size();
                let (x, _) = self.cursor_pos();
                self.set_cursor_pos(x, height - 1);
                self.edit_cache.clear();
            }
            BitFontFocusedPanel::CharSet => {
                // Move to row 15 (last row in 16x16 grid)
                self.state.clear_charset_selection();
                let (x, _) = self.charset_cursor();
                self.set_charset_cursor(x, 15);
                self.selector_cache.clear();
            }
        }
        Task::none()
    }

    /// Build the editor view
    ///
    /// The optional `chat_panel` parameter is accepted for API consistency but
    /// currently ignored since bitfont editor doesn't support collaboration.
    pub fn view(&self, _chat_panel: Option<Element<'_, BitFontEditorMessage>>) -> Element<'_, BitFontEditorMessage> {
        // If preview mode is active, show the preview instead of the editor
        if self.show_preview {
            return self.view_preview();
        }

        // === LEFT SIDEBAR (like ANSI editor) ===
        // Use theme's main area background color
        let bg_weakest = main_area_background(&Theme::Dark);
        let icon_color = Theme::Dark.extended_palette().background.base.text;

        // Palette grid + Tool panel
        let palette_view = self.palette_grid.view_with_width(SIDEBAR_WIDTH, None).map(BitFontEditorMessage::PaletteGrid);
        let tool_panel = self
            .tool_panel
            .view_with_config(SIDEBAR_WIDTH, bg_weakest, icon_color)
            .map(BitFontEditorMessage::ToolPanel);
        let left_sidebar = column![palette_view, tool_panel,].spacing(4);

        // === TOP TOOLBAR (color switcher + tool options) ===
        let color_switcher = self.top_toolbar.view_color_switcher().map(BitFontEditorMessage::TopToolbar);

        let top_toolbar_panel = self.top_toolbar.view().map(BitFontEditorMessage::TopToolbar);

        let toolbar_height = SWITCHER_SIZE;

        let top_toolbar = row![color_switcher, top_toolbar_panel,].spacing(4);

        let (font_width, font_height) = self.state.font_size();
        let use_letter_spacing = self.use_letter_spacing();

        // Calculate dynamic scales based on font size and available height
        let (edit_scale, _charset_scale) = self.calculate_grid_scales();
        let scaled_cell_size = EDIT_CELL_SIZE * edit_scale;
        let scaled_cell_gap = EDIT_CELL_BORDER * edit_scale;

        // === CENTER: Edit grid canvas ===
        // Add extra column for 9-dot mode if letter spacing is enabled
        let display_width = if use_letter_spacing && font_width == 8 {
            font_width + 1 // 9-dot mode
        } else {
            font_width
        };
        let edit_grid_width = RULER_SIZE + (scaled_cell_size + scaled_cell_gap) * display_width as f32;
        let edit_grid_height = RULER_SIZE + (scaled_cell_size + scaled_cell_gap) * font_height as f32;

        // Get colors from top toolbar
        let fg_color = self.top_toolbar.foreground;
        let bg_color = self.top_toolbar.background;

        let edit_canvas = Canvas::new(EditGridCanvas {
            editor: self,
            fg_color,
            bg_color,
        })
        .width(Length::Fixed(edit_grid_width))
        .height(Length::Fixed(edit_grid_height));

        // Character Set (16x16 grid, positioned right of editor)
        let char_set = self.view_glyph_selector();

        // Tile View (8x8 grid of current character, top-right)
        let tile_view = self.view_tile_area();

        // === LAYOUT ===
        // Top row: Full-width toolbar
        // Middle row: Left sidebar | Edit Canvas | Character Set | Tile View

        // Edit canvas in container with label
        // Convert selected char to CP437 Unicode representation
        let selected_char_code = self.selected_char() as u8;
        let unicode_char = CP437_TO_UNICODE.get(selected_char_code as usize).copied().unwrap_or(selected_char_code as char);
        let edit_title = format!("0x{:02X}: {}", selected_char_code, unicode_char);

        let edit_area = column![
            Space::new().height(Length::Fill),
            text(edit_title).size(18).align_x(Horizontal::Center).width(Length::Fixed(edit_grid_width)),
            edit_canvas,
            Space::new().height(Length::Fill),
        ]
        .spacing(4);

        let right_sidebar: iced::widget::Column<'_, BitFontEditorMessage> = column![Space::new().height(Length::Fixed(8.0)), tile_view]
            .width(Length::Fixed(140.0))
            .align_x(Horizontal::Center);

        let middle_row = row![
            // Left sidebar
            container(left_sidebar).width(Length::Fixed(SIDEBAR_WIDTH)),
            container(row![
                // Edit canvas - centered
                Space::new().width(Length::Fill),
                edit_area,
                // More space between controls
                Space::new().width(Length::Fixed(40.0)),
                // Character Set (right of editor)
                char_set,
                Space::new().width(Length::Fill),
            ])
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme::main_area_background(theme))),
                ..Default::default()
            }),
            // Tile View (top-right)
            container(right_sidebar).width(Length::Fixed(140.0)),
        ]
        .spacing(4);

        let main_content: Element<'_, BitFontEditorMessage> = column![
            // Top toolbar
            container(top_toolbar)
                .width(Length::Fill)
                .height(Length::Fixed(toolbar_height))
                .style(container::rounded_box),
            // Middle content
            container(middle_row).width(Length::Fill).height(Length::Fill).style(container::bordered_box),
        ]
        .spacing(0)
        .into();

        // Dialog is now rendered by MainWindow's DialogStack
        main_content
    }

    /// Build the glyph selector view - compact 16x16 grid with hex labels
    fn view_glyph_selector(&self) -> Element<'_, BitFontEditorMessage> {
        // Get dynamic scale based on font size and available height
        let (_edit_scale, charset_scale) = self.calculate_grid_scales();

        // Add extra column for 9-dot mode if letter spacing is enabled
        let (font_width, font_height) = self.state.font_size();
        let display_width = if self.use_letter_spacing() && font_width == 8 {
            font_width + 1
        } else {
            font_width
        };
        let cell_width = display_width as f32 * charset_scale;
        let cell_height = font_height as f32 * charset_scale;

        // Get colors from top toolbar
        let fg_color = self.top_toolbar.foreground;
        let bg_color = self.top_toolbar.background;

        // Use same ruler size as edit grid for consistency
        let label_size = style::RULER_SIZE;

        // Total grid size: label + 16 chars (matching edit grid layout)
        let grid_width = label_size + 16.0 * cell_width;
        let grid_height = label_size + 16.0 * cell_height;

        // Create canvas for the entire character set
        let charset_canvas = Canvas::new(CharSetCanvas {
            editor: self,
            fg_color,
            bg_color,
            cell_width,
            cell_height,
            label_size,
        })
        .width(Length::Fixed(grid_width))
        .height(Length::Fixed(grid_height));

        // Show font name if available, otherwise "Character Set"
        let font_name = self.state.font_name();
        let charset_title = if font_name.is_empty() {
            "Character Set".to_string()
        } else {
            font_name.to_string()
        };

        column![
            Space::new().height(Length::Fill),
            text(charset_title).size(18).align_x(Horizontal::Center).width(Length::Fixed(grid_width)),
            charset_canvas,
            Space::new().height(Length::Fill),
        ]
        .spacing(4)
        .into()
    }

    /// Build the tile view - 8x8 grid of the current character
    fn view_tile_area(&self) -> Element<'_, BitFontEditorMessage> {
        // Use same dynamic scale as character set
        let (_edit_scale, charset_scale) = self.calculate_grid_scales();

        // Add extra column for 9-dot mode if letter spacing is enabled
        let (font_width, font_height) = self.state.font_size();
        let display_width = if self.use_letter_spacing() && font_width == 8 {
            font_width + 1
        } else {
            font_width
        };
        let cell_width = display_width as f32 * charset_scale;
        let cell_height = font_height as f32 * charset_scale;

        // Get colors from top toolbar
        let fg_color = self.top_toolbar.foreground;
        let bg_color = self.top_toolbar.background;

        // 8x8 grid of the current character
        let grid_size = 8;
        let grid_width = grid_size as f32 * cell_width;
        let grid_height = grid_size as f32 * cell_height;

        let tile_canvas = Canvas::new(TileViewCanvas {
            editor: self,
            fg_color,
            bg_color,
            cell_width,
            cell_height,
            grid_size,
        })
        .width(Length::Fixed(grid_width))
        .height(Length::Fixed(grid_height));

        column![text("Tile View").size(18), tile_canvas,].align_x(Horizontal::Center).spacing(4).into()
    }

    /// Build the preview view - displays font in terminal-like screen
    fn view_preview(&self) -> Element<'_, BitFontEditorMessage> {
        let preview_element: Element<'_, BitFontEditorMessage> = if let Some(terminal) = &self.preview_terminal {
            TerminalView::show_with_effects(terminal, self.preview_monitor.clone(), None).map(BitFontEditorMessage::PreviewTerminal)
        } else {
            text("Preparing preview...").size(12).into()
        };

        container(column![text("Font Preview - Press any key to exit").size(14), preview_element,].spacing(8))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(iced::Background::Color(main_area_background(theme))),
                ..Default::default()
            })
            .into()
    }

    /// Get status information for the status bar
    pub fn status_info(&self) -> (String, String, String) {
        (
            {
                let ch = self.selected_char();
                format!("Char: {} (0x{:02X})", ch, ch as u32)
            },
            {
                let (width, height) = self.state.font_size();
                format!("{}×{}", width, height)
            },
            format!("Undo: {} Redo: {}", self.state.undo_stack_len(), self.state.redo_stack_len()),
        )
    }

    /// Check whether the current font has unsaved changes
    #[allow(dead_code)]
    pub fn is_modified(&self) -> bool {
        self.state.is_dirty()
    }

    /// Access current file path if loaded from disk
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.state.file_path()
    }

    /// Set the file path
    pub fn set_file_path(&mut self, path: std::path::PathBuf) {
        self.state.set_file_path(Some(path));
    }

    /// Save the font to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        // Build the font from current state
        let font = self.state.build_font();

        // Determine format from extension
        let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

        let bytes = if ext == "yaff" {
            // YAFF format is read-only for now, save as text representation
            // Note: libyaff doesn't have a to_yaff_bytes method yet
            return Err("YAFF export is not yet supported. Please save as .psf instead.".to_string());
        } else {
            // Default to PSF2 binary format
            font.to_psf2_bytes().map_err(|e| e.to_string())?
        };

        std::fs::write(path, bytes).map_err(|e| e.to_string())?;

        // Mark as clean after successful save
        self.state.mark_clean();

        Ok(())
    }

    pub(crate) fn handle_event(&self, event: &iced::Event) -> Option<BitFontEditorMessage> {
        // Font size dialog events are now handled by MainWindow's DialogStack

        // If in preview mode, any key or mouse click exits the preview
        if self.show_preview {
            match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { .. }) => {
                    return Some(BitFontEditorMessage::HidePreview);
                }
                iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => {
                    return Some(BitFontEditorMessage::HidePreview);
                }
                _ => {}
            }
            return None;
        }

        // Menu commands are handled by MainWindow.handle_event() - not here
        // This method only handles editor-specific shortcuts (tools, navigation)

        // Handle keyboard shortcuts directly (editor-specific, not in menu)
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
            match key {
                // Tool selection shortcuts (c, s, l, r, g are mapped differently in bitfont)
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("c") && !modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectTool(BitFontTool::Click));
                }
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("s") && !modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectTool(BitFontTool::Select));
                }
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("l") && !modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectTool(BitFontTool::Line));
                }
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("r") && !modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectTool(BitFontTool::RectangleOutline));
                }
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("g") && !modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectTool(BitFontTool::Fill));
                }

                // Character navigation
                Key::Character(c) if c.as_str() == "+" || c.as_str() == "=" => {
                    return Some(BitFontEditorMessage::NextChar);
                }
                Key::Character(c) if c.as_str() == "-" => {
                    return Some(BitFontEditorMessage::PrevChar);
                }

                // Select all (Ctrl+A)
                Key::Character(c) if c.as_str().eq_ignore_ascii_case("a") && modifiers.command() => {
                    return Some(BitFontEditorMessage::SelectAll);
                }

                // Clear selection (Escape)
                Key::Named(keyboard::key::Named::Escape) => {
                    return Some(BitFontEditorMessage::ClearSelection);
                }

                _ => {}
            }
        }
        None
    }
}

impl Default for BitFontEditor {
    fn default() -> Self {
        Self::new()
    }
}
