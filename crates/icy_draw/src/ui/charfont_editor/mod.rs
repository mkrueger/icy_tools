//! CharFont (TDF) Editor Mode
//!
//! This module contains the TDF font editor with:
//! - Left sidebar: Font list, character selector
//! - Top bar: Font name, spacing, font type info
//! - Center: Split view - top: character preview, bottom: charset grid
//! - Color switcher and ANSI toolbar for editing

mod charset_canvas;
pub mod menu_bar;
mod top_bar;

pub use charset_canvas::*;
pub use top_bar::*;

use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    Element, Length, Task, Theme,
    keyboard::Modifiers,
    widget::{Space, button, canvas, column, container, row, scrollable, text},
};
use icy_engine::char_set::TdfBufferRenderer;
use icy_engine::{AttributedChar, BitFont, Layer, Screen, Size, TextAttribute, TextBuffer};
use icy_engine_edit::charset::{CharSetEditState, CharSetFocusedPanel, TdfFont, TdfFontExt, load_tdf_fonts};
use icy_engine_edit::{EditState, UndoState};
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::{MonitorSettings, ScalingMode, Terminal, TerminalView};
use parking_lot::Mutex;
use retrofont::RenderOptions;

use crate::ui::SharedOptions;
use crate::ui::ansi_editor::{ColorSwitcher, ColorSwitcherMessage, PaletteGrid, PaletteGridMessage, ToolPanel, ToolPanelMessage};

/// Direction for arrow key navigation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Messages for the CharFont editor
#[derive(Clone, Debug)]
pub enum CharFontEditorMessage {
    /// Top bar messages
    TopBar(TopBarMessage),
    /// Color switcher messages
    ColorSwitcher(ColorSwitcherMessage),
    /// Palette grid messages
    PaletteGrid(PaletteGridMessage),
    /// Tool panel messages
    ToolPanel(ToolPanelMessage),
    /// Select a font from the list
    SelectFont(usize),
    /// Select a character to edit
    SelectChar(char),
    /// Select a character at a specific grid position (also sets focus and cursor)
    SelectCharAt(char, i32, i32),
    /// Clone the current font
    CloneFont,
    /// Delete the current font
    DeleteFont,
    /// Clear the current character
    ClearChar,
    /// Font name changed
    FontNameChanged(String),
    /// Spacing changed
    SpacingChanged(i32),
    /// Animation tick
    Tick(f32),
    /// Move charset cursor by delta
    MoveCharsetCursor(i32, i32),
    /// Set charset cursor to absolute position
    SetCharsetCursor(i32, i32),
    /// Extend charset selection with shift+arrows
    ExtendCharsetSelection(i32, i32, bool),
    /// Set charset selection lead position (for mouse drag)
    SetCharsetSelectionLead(i32, i32, bool),
    /// Clear charset selection
    ClearCharsetSelection,
    /// Select character at charset cursor (Space/Enter)
    SelectCharAtCursor,
    /// Switch focus to next panel (Tab)
    FocusNextPanel,
    /// Set focus to a specific panel
    SetFocusedPanel(CharSetFocusedPanel),
    /// Arrow key pressed with modifiers
    HandleArrow(ArrowDirection, Modifiers),
    /// Home key
    HandleHome,
    /// End key
    HandleEnd,
    /// Page up
    HandlePageUp,
    /// Page down
    HandlePageDown,
    /// Confirm action (Space/Enter)
    HandleConfirm,
    /// Cancel action (Escape)
    HandleCancel,
    /// Delete selected characters
    HandleDelete,
}

/// The CharFont (TDF) editor component
pub struct CharFontEditor {
    /// Unique ID for this editor
    pub id: u64,
    /// The CharSet edit state (model layer from icy_engine_edit)
    charset_state: CharSetEditState,
    /// The buffer font for rendering characters
    render_font: BitFont,
    /// The edit state for the character being edited (for pixel editing)
    edit_state: Arc<Mutex<EditState>>,
    /// Color switcher (FG/BG display)
    color_switcher: ColorSwitcher,
    /// Palette grid
    palette_grid: PaletteGrid,
    /// Tool panel
    tool_panel: ToolPanel,
    /// Top bar state
    top_bar: TopBar,
    /// Undo stack length tracking
    undostack_len: usize,
    /// Last update preview undo length
    last_update_preview: usize,
    /// Shared options
    pub options: Arc<Mutex<SharedOptions>>,
    /// Terminal for preview rendering
    preview_terminal: Terminal,
    /// Monitor settings for preview (200% zoom)
    preview_monitor: MonitorSettings,
}

static mut NEXT_ID: u64 = 0;

impl CharFontEditor {
    /// Create a new empty CharFont editor
    pub fn new(options: Arc<Mutex<SharedOptions>>) -> Self {
        let charset_state = CharSetEditState::new();
        Self::with_charset_state(charset_state, options)
    }

    /// Create a CharFont editor from CharSetEditState
    fn with_charset_state(charset_state: CharSetEditState, options: Arc<Mutex<SharedOptions>>) -> Self {
        let id = unsafe {
            NEXT_ID = NEXT_ID.wrapping_add(1);
            NEXT_ID
        };

        // Load the TDF font for rendering
        let render_font = BitFont::default();

        // Create edit buffer for the character
        let mut buffer = TextBuffer::create((30, 12));
        buffer.set_font(0, render_font.clone());
        set_up_buffer(&mut buffer);

        let palette = buffer.palette.clone();
        let edit_state = Arc::new(Mutex::new(EditState::from_buffer(buffer)));

        let mut palette_grid = PaletteGrid::new();
        palette_grid.sync_palette(&palette);

        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        // Create terminal for preview with 200% zoom
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(EditState::from_buffer({
            let mut buf = TextBuffer::create((30, 12));
            buf.set_font(0, render_font.clone());
            set_up_buffer(&mut buf);
            buf
        }))));
        let preview_terminal = Terminal::new(screen);

        let mut preview_monitor = MonitorSettings::default();
        preview_monitor.scaling_mode = ScalingMode::Manual(2.0); // 200% zoom

        let mut editor = Self {
            id,
            charset_state,
            render_font,
            edit_state,
            color_switcher,
            palette_grid,
            tool_panel: ToolPanel::new(),
            top_bar: TopBar::new(),
            undostack_len: 0,
            last_update_preview: 0,
            options,
            preview_terminal,
            preview_monitor,
        };

        editor.update_selected_char();
        editor
    }

    /// Create a CharFont editor from TDF fonts
    pub fn with_fonts(fonts: Vec<TdfFont>, file_path: Option<PathBuf>, options: Arc<Mutex<SharedOptions>>) -> Self {
        let charset_state = CharSetEditState::with_fonts(fonts, file_path);
        Self::with_charset_state(charset_state, options)
    }

    /// Create a CharFont editor with a file
    pub fn with_file(path: PathBuf, options: Arc<Mutex<SharedOptions>>) -> anyhow::Result<Self> {
        let charset_state = CharSetEditState::load_from_file(path)?;
        Ok(Self::with_charset_state(charset_state, options))
    }

    /// Get the file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.charset_state.file_path()
    }

    /// Set the file path (for session restore)
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.charset_state.set_file_path(Some(path));
    }

    /// Get the document title for display
    pub fn title(&self) -> String {
        let file_name = self
            .charset_state
            .file_path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");

        let modified = if self.charset_state.is_dirty() { " •" } else { "" };
        format!("{}{}", file_name, modified)
    }

    /// Get undo stack length for dirty tracking
    pub fn undo_stack_len(&self) -> usize {
        self.undostack_len
    }

    /// Save the document to the given path
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.save_old_selected_char();
        self.charset_state.save(path)
    }

    /// Get bytes for autosave
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        self.charset_state.get_autosave_bytes()
    }

    /// Load from an autosave file
    pub fn load_from_autosave(autosave_path: &std::path::Path, original_path: PathBuf, options: Arc<Mutex<SharedOptions>>) -> anyhow::Result<Self> {
        let data = std::fs::read(autosave_path)?;
        let fonts = load_tdf_fonts(&data)?;
        if fonts.is_empty() {
            anyhow::bail!("No fonts found in autosave file");
        }

        let mut charset_state = CharSetEditState::with_fonts(fonts, Some(original_path));
        charset_state.set_dirty(true);
        Ok(Self::with_charset_state(charset_state, options))
    }

    /// Check if this editor needs animation updates
    pub fn needs_animation(&self) -> bool {
        self.color_switcher.needs_animation() || self.tool_panel.needs_animation()
    }

    /// Get undo description
    pub fn undo_description(&self) -> Option<String> {
        // First check charset_state undo, then edit_state
        self.charset_state.undo_description().or_else(|| self.edit_state.lock().undo_description())
    }

    /// Get redo description
    pub fn redo_description(&self) -> Option<String> {
        self.charset_state.redo_description().or_else(|| self.edit_state.lock().redo_description())
    }

    /// Undo
    pub fn undo(&mut self) {
        // Try charset_state first, then edit_state
        if !self.charset_state.undo() {
            let mut state = self.edit_state.lock();
            let _ = state.undo();
        }
    }

    /// Redo
    pub fn redo(&mut self) {
        if !self.charset_state.redo() {
            let mut state = self.edit_state.lock();
            let _ = state.redo();
        }
    }

    /// Save the currently edited character back to the font
    /// TODO: Implement conversion from TextBuffer back to retrofont::Glyph
    fn save_old_selected_char(&mut self) {
        let state = self.edit_state.lock();
        if state.undo_stack_len() == 0 {
            return;
        }
        drop(state);

        self.undostack_len += 1;

        // TODO: Convert edited buffer back to retrofont::Glyph
        // This requires implementing buffer -> GlyphPart conversion
        // For now, editing is view-only until this is implemented
    }

    /// Update the edit buffer with the selected character
    fn update_selected_char(&mut self) {
        self.save_old_selected_char();

        let font = match self.charset_state.selected_font() {
            Some(f) => f,
            None => return,
        };

        let selected_char = self.charset_state.selected_char();

        let mut state = self.edit_state.lock();
        let buffer = state.get_buffer_mut();
        set_up_buffer(buffer);
        state.set_current_layer(0);
        state.get_caret_mut().set_position(icy_engine::Position::new(0, 0));

        if let Some(ch) = selected_char {
            // Use TdfBufferRenderer to render the glyph
            if let Some(glyph) = font.glyph(ch) {
                let buffer = state.get_buffer_mut();
                let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                let options = RenderOptions::default();
                let _ = glyph.render(&mut renderer, &options);
            }
        }

        // Mark buffer as dirty to trigger re-render
        state.get_buffer_mut().mark_dirty();
        state.get_undo_stack().lock().unwrap().clear();

        // Update the preview terminal with the rendered character
        drop(state);
        self.update_preview_terminal();
    }

    /// Update the preview terminal to show the currently selected character
    fn update_preview_terminal(&mut self) {
        let font = match self.charset_state.selected_font() {
            Some(f) => f,
            None => return,
        };

        let selected_char = self.charset_state.selected_char();

        // Lock the preview terminal's screen and update it
        let mut screen = self.preview_terminal.screen.lock();

        // The screen is a Box<dyn Screen>, we need to cast it to EditState to access the buffer
        if let Some(edit_state) = screen.as_any_mut().downcast_mut::<EditState>() {
            let buffer = edit_state.get_buffer_mut();
            set_up_buffer(buffer);
            edit_state.set_current_layer(0);
            edit_state.get_caret_mut().set_position(icy_engine::Position::new(0, 0));

            if let Some(ch) = selected_char {
                // Use TdfBufferRenderer to render the glyph
                if let Some(glyph) = font.glyph(ch) {
                    let buffer = edit_state.get_buffer_mut();
                    let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                    let options = RenderOptions::default();
                    let _ = glyph.render(&mut renderer, &options);
                }
            }

            // Mark buffer as dirty to trigger re-render
            edit_state.get_buffer_mut().mark_dirty();
            edit_state.get_undo_stack().lock().unwrap().clear();
        }
    }

    /// Update the editor state
    pub fn update(&mut self, message: CharFontEditorMessage) -> Task<CharFontEditorMessage> {
        match message {
            CharFontEditorMessage::TopBar(msg) => {
                match &msg {
                    TopBarMessage::FontNameChanged(name) => {
                        self.charset_state.set_font_name(name.clone());
                        self.undostack_len += 1;
                    }
                    TopBarMessage::SpacingChanged(spacing) => {
                        self.charset_state.set_font_spacing(*spacing);
                        self.undostack_len += 1;
                    }
                }
                self.top_bar.update(msg).map(CharFontEditorMessage::TopBar)
            }
            CharFontEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        self.color_switcher.start_swap_animation();
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        let mut state = self.edit_state.lock();
                        let caret = state.get_caret_mut();
                        let fg = caret.attribute.foreground();
                        let bg = caret.attribute.background();
                        caret.attribute.set_foreground(bg);
                        caret.attribute.set_background(fg);
                        drop(state);
                        self.palette_grid.set_foreground(bg);
                        self.palette_grid.set_background(fg);
                        self.color_switcher.confirm_swap();
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        let mut state = self.edit_state.lock();
                        let caret = state.get_caret_mut();
                        caret.attribute.set_foreground(7);
                        caret.attribute.set_background(0);
                        drop(state);
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                    }
                    ColorSwitcherMessage::Tick(delta) => {
                        if self.color_switcher.tick(delta) {
                            return Task::done(CharFontEditorMessage::ColorSwitcher(ColorSwitcherMessage::AnimationComplete));
                        }
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        let mut state = self.edit_state.lock();
                        state.get_caret_mut().attribute.set_foreground(color);
                        drop(state);
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        let mut state = self.edit_state.lock();
                        state.get_caret_mut().attribute.set_background(color);
                        drop(state);
                        self.palette_grid.set_background(color);
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::ToolPanel(msg) => {
                match &msg {
                    ToolPanelMessage::Tick(delta) => {
                        self.tool_panel.tick(*delta);
                    }
                    _ => {
                        let _ = self.tool_panel.update(msg);
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::SelectFont(idx) => {
                self.save_old_selected_char();
                self.charset_state.select_font(idx);
                self.update_selected_char();
                Task::none()
            }
            CharFontEditorMessage::SelectChar(ch) => {
                self.charset_state.select_char(ch);
                self.update_selected_char();
                Task::none()
            }
            CharFontEditorMessage::SelectCharAt(_ch, col, row) => {
                self.charset_state.select_char_at(col, row);
                self.update_selected_char();
                Task::none()
            }
            CharFontEditorMessage::CloneFont => {
                self.charset_state.clone_font();
                self.update_selected_char();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::DeleteFont => {
                self.charset_state.delete_font();
                self.update_selected_char();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::ClearChar => {
                self.charset_state.clear_selected_char();
                self.update_selected_char();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::FontNameChanged(name) => {
                self.charset_state.set_font_name(name);
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::SpacingChanged(spacing) => {
                self.charset_state.set_font_spacing(spacing);
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::Tick(delta) => {
                self.color_switcher.tick(delta);
                self.tool_panel.tick(delta);

                // Check if preview needs updating
                let u = self.edit_state.lock().undo_stack_len();
                if self.last_update_preview != u {
                    self.last_update_preview = u;
                    self.save_old_selected_char();
                }
                Task::none()
            }
            CharFontEditorMessage::MoveCharsetCursor(dx, dy) => {
                self.charset_state.move_charset_cursor(dx, dy);
                Task::none()
            }
            CharFontEditorMessage::SetCharsetCursor(col, row) => {
                self.charset_state.set_charset_cursor(col, row);
                Task::none()
            }
            CharFontEditorMessage::ExtendCharsetSelection(dx, dy, is_rectangle) => {
                self.charset_state.extend_charset_selection(dx, dy, is_rectangle);
                Task::none()
            }
            CharFontEditorMessage::SetCharsetSelectionLead(col, row, _is_rectangle) => {
                // This is handled via extend_charset_selection in the state
                self.charset_state.set_charset_cursor(col, row);
                Task::none()
            }
            CharFontEditorMessage::ClearCharsetSelection => {
                self.charset_state.clear_charset_selection();
                Task::none()
            }
            CharFontEditorMessage::SelectCharAtCursor => {
                self.charset_state.select_char_at_cursor();
                self.update_selected_char();
                Task::none()
            }
            CharFontEditorMessage::FocusNextPanel => {
                self.charset_state.focus_next_panel();
                Task::none()
            }
            CharFontEditorMessage::SetFocusedPanel(panel) => {
                self.charset_state.set_focused_panel(panel);
                Task::none()
            }
            CharFontEditorMessage::HandleArrow(direction, modifiers) => {
                let (dx, dy) = match direction {
                    ArrowDirection::Up => (0, -1),
                    ArrowDirection::Down => (0, 1),
                    ArrowDirection::Left => (-1, 0),
                    ArrowDirection::Right => (1, 0),
                };

                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        if modifiers.shift() {
                            let is_rectangle = modifiers.alt();
                            return Task::done(CharFontEditorMessage::ExtendCharsetSelection(dx, dy, is_rectangle));
                        } else {
                            return Task::done(CharFontEditorMessage::MoveCharsetCursor(dx, dy));
                        }
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor movement
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandleHome => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        self.charset_state.charset_home();
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandleEnd => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        self.charset_state.charset_end();
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandlePageUp => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        self.charset_state.charset_page_up();
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandlePageDown => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        self.charset_state.charset_page_down();
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandleConfirm => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        return Task::done(CharFontEditorMessage::SelectCharAtCursor);
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit confirm
                    }
                }
                Task::none()
            }
            CharFontEditorMessage::HandleCancel => {
                self.charset_state.clear_charset_selection();
                Task::none()
            }
            CharFontEditorMessage::HandleDelete => {
                match self.charset_state.focused_panel() {
                    CharSetFocusedPanel::CharSet => {
                        self.charset_state.delete_selected_chars();
                        self.update_selected_char();
                        self.undostack_len += 1;
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit delete
                    }
                }
                Task::none()
            }
        }
    }

    /// Render the editor view
    pub fn view(&self) -> Element<'_, CharFontEditorMessage> {
        // === RIGHT PANEL: Font list ===
        let mut font_list_content: Vec<Element<'_, CharFontEditorMessage>> = Vec::new();
        for (i, font) in self.charset_state.fonts().iter().enumerate() {
            let is_selected = i == self.charset_state.selected_font_index();
            let btn = button(text(&font.name).size(12))
                .on_press(CharFontEditorMessage::SelectFont(i))
                .width(Length::Fill)
                .style(if is_selected { button::primary } else { button::secondary });
            font_list_content.push(btn.into());
        }

        let font_list = scrollable(column(font_list_content).spacing(2).width(Length::Fill)).height(Length::Fill);

        // Font actions
        let font_actions = row![
            button(text("🗑").size(14)).on_press_maybe(if self.charset_state.font_count() > 1 {
                Some(CharFontEditorMessage::DeleteFont)
            } else {
                None
            }),
            button(text(crate::fl!("tdf-editor-clone_button")).size(12)).on_press(CharFontEditorMessage::CloneFont),
        ]
        .spacing(4);

        let right_panel = column![text(crate::fl!("tdf-editor-font_name_label")).size(14), font_list, font_actions,]
            .spacing(4)
            .width(Length::Fixed(180.0));

        // === TOP BAR ===
        let top_bar = if let Some(font) = self.charset_state.selected_font() {
            self.top_bar.view(font).map(CharFontEditorMessage::TopBar)
        } else {
            container(text(crate::fl!("tdf-editor-no_font_selected_label")).size(16))
                .center_x(Length::Fill)
                .height(Length::Fixed(60.0))
                .into()
        };

        // === COLOR SWITCHER AND TOOLBAR ===
        let (caret_fg, caret_bg) = {
            let state = self.edit_state.lock();
            let caret = state.get_caret();
            (caret.attribute.foreground(), caret.attribute.background())
        };

        let color_switcher = self.color_switcher.view(caret_fg, caret_bg).map(CharFontEditorMessage::ColorSwitcher);
        let palette_view = self.palette_grid.view_with_width(64.0).map(CharFontEditorMessage::PaletteGrid);
        let bg_weakest = main_area_background(&Theme::Dark);
        let tool_panel = self.tool_panel.view_with_config(64.0, bg_weakest).map(CharFontEditorMessage::ToolPanel);

        let toolbar = row![color_switcher, Space::new().width(Length::Fill)].spacing(4);

        let left_tools = column![palette_view, tool_panel].spacing(4);

        // === CENTER TOP: Character preview/editor using Terminal ===
        let char_preview: Element<'_, CharFontEditorMessage> = if let Some(ch) = self.charset_state.selected_char() {
            let preview_label = if let Some(font) = self.charset_state.selected_font() {
                if font.has_char(ch) {
                    format!("Editing: '{}'", ch)
                } else {
                    format!("New char: '{}'", ch)
                }
            } else {
                format!("Char: '{}'", ch)
            };

            // Render the terminal preview
            let terminal_view = TerminalView::show_with_effects(&self.preview_terminal, self.preview_monitor.clone()).map(|_| CharFontEditorMessage::Tick(0.0));

            container(column![text(preview_label).size(14), terminal_view,].spacing(8))
                .center_x(Length::Fill)
                .height(Length::FillPortion(2))
                .style(container::bordered_box)
                .into()
        } else {
            container(text("Select a character from the grid below").size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(container::bordered_box)
                .into()
        };

        // === CENTER BOTTOM: CharSet canvas ===
        let font_ref = self.charset_state.selected_font();

        let cell_width = 24.0;
        let cell_height = 24.0;
        let label_size = 20.0;
        let canvas_width = label_size + 16.0 * cell_width;
        let canvas_height = label_size + 6.0 * cell_height;

        let (cursor_col, cursor_row) = self.charset_state.charset_cursor();
        let selection = self
            .charset_state
            .charset_selection()
            .map(|(a, l, r)| (iced::Point::new(a.x, a.y), iced::Point::new(l.x, l.y), r));

        let charset_canvas = canvas::Canvas::new(CharSetCanvas {
            font: font_ref,
            selected_char: self.charset_state.selected_char(),
            cursor_col,
            cursor_row,
            is_focused: self.charset_state.focused_panel() == CharSetFocusedPanel::CharSet,
            selection,
            cell_width,
            cell_height,
            label_size,
        })
        .width(Length::Fixed(canvas_width))
        .height(Length::Fixed(canvas_height));

        let charset_section = container(charset_canvas).style(container::bordered_box).center_x(Length::Fill);

        // === CENTER AREA: Split view ===
        let center_area = column![char_preview, charset_section,].spacing(8);

        // === LAYOUT ===
        let main_area = column![toolbar, row![left_tools, center_area,].spacing(4),].spacing(4);

        let content = row![column![top_bar, main_area,].spacing(4), container(right_panel).style(container::bordered_box),].spacing(4);

        container(content).padding(4).width(Length::Fill).height(Length::Fill).into()
    }

    /// Get status bar information
    pub fn status_info(&self) -> (String, String, String) {
        let left = format!("Font {}/{}", self.charset_state.selected_font_index() + 1, self.charset_state.font_count());
        let center = if let Some(ch) = self.charset_state.selected_char() {
            format!("Char: '{}' (0x{:02X})", ch, ch as u8)
        } else {
            "No char selected".to_string()
        };
        let right = if let Some(font) = self.charset_state.selected_font() {
            format!("{:?}", font.font_type)
        } else {
            String::new()
        };
        (left, center, right)
    }
}

/// Set up the edit buffer for TDF editing (single layer)
fn set_up_buffer(buffer: &mut TextBuffer) {
    buffer.layers.clear();
    let char_size = Size::new(30, 12);

    // Single edit layer
    let mut layer = Layer::new("edit", char_size);
    layer.properties.has_alpha_channel = false;
    layer.properties.is_position_locked = true;
    for y in 0..char_size.height {
        for x in 0..char_size.width {
            layer.set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
        }
    }
    buffer.layers.push(layer);
}
