//! CharFont (TDF) Editor Mode
//!
//! This module contains the TDF font editor with:
//! - Left sidebar: Font list, character selector
//! - Top bar: Font name, spacing, font type info
//! - Center: Split view - top: character preview, bottom: charset grid
//! - Color switcher and ANSI toolbar for editing

mod charset_canvas;
pub mod menu_bar;
mod outline_style_preview;

pub use charset_canvas::*;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use iced::{
    Alignment, Element, Length, Task, Theme,
    keyboard::Modifiers,
    widget::{button, canvas, column, container, row, scrollable, text},
};
use icy_engine::Screen;
use icy_engine::char_set::TdfBufferRenderer;
use icy_engine::{AttributedChar, BitFont, Layer, Size, TextAttribute, TextBuffer, TextPane};
use icy_engine_edit::EditState;
use icy_engine_edit::UndoState;
use icy_engine_edit::charset::{CharSetEditState, CharSetFocusedPanel, TdfFontType, load_tdf_fonts};
use icy_engine_gui::TerminalMessage;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::ui::DialogStack;
use parking_lot::{Mutex, RwLock};
use retrofont::{RenderOptions, transform_outline};

use crate::SharedFontLibrary;
use crate::ui::Options;
use crate::ui::editor::ansi::constants;
use crate::ui::editor::ansi::widget::canvas::CanvasView;
use crate::ui::editor::ansi::{
    AnsiEditorCore, AnsiEditorCoreMessage, ColorSwitcher, ColorSwitcherMessage, PaletteGrid, PaletteGridMessage, TdfFontSelectorDialog, ToolPanel,
    ToolPanelMessage, tool_registry, tools,
};
use crate::ui::main_window::Message;

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

    // ═══════════════════════════════════════════════════════════════════════════
    // TDF Font Selector Dialog
    // ═══════════════════════════════════════════════════════════════════════════
    /// Open TDF font selector dialog
    OpenTdfFontSelector,
    /// TDF font selector dialog messages
    TdfFontSelector(crate::ui::editor::ansi::TdfFontSelectorMessage),

    // ═══════════════════════════════════════════════════════════════════════════
    // ANSI Editor Core Messages (for editing TDF glyphs)
    // ═══════════════════════════════════════════════════════════════════════════
    /// Forward messages to the embedded AnsiEditorCore for glyph editing
    AnsiEditor(crate::ui::editor::ansi::AnsiEditorCoreMessage),

    // ═══════════════════════════════════════════════════════════════════════════
    // Outline Style Selection
    // ═══════════════════════════════════════════════════════════════════════════
    /// Select an outline style (0-18)
    SelectOutlineStyle(usize),

    /// Outline preview canvas messages (right-side preview for outline fonts)
    OutlinePreviewCanvas(TerminalMessage),
}

/// The CharFont (TDF) editor component
pub struct CharFontEditor {
    /// The CharSet edit state (model layer from icy_engine_edit)
    charset_state: CharSetEditState,
    /// The ANSI editor core for editing TDF glyphs
    ansi_core: AnsiEditorCore,
    /// Tool registry for managing tool instances
    tool_registry: Rc<RefCell<tool_registry::ToolRegistry>>,
    /// Color switcher (FG/BG display)
    color_switcher: ColorSwitcher,
    /// Palette grid
    palette_grid: PaletteGrid,
    /// Tool panel
    tool_panel: ToolPanel,
    /// Font library reference (needed for recreating tool registry)
    font_library: SharedFontLibrary,
    /// Undo stack length tracking
    undostack_len: usize,
    /// Last update preview undo length
    last_update_preview: usize,
    /// Currently selected outline style (0-18) for outline font preview
    selected_outline_style: usize,

    /// Separate preview screen + canvas for outline fonts (egui-like preview buffer)
    outline_preview_screen: Arc<Mutex<Box<dyn Screen>>>,
    outline_preview_canvas: CanvasView,
}

impl CharFontEditor {
    /// Create a new empty CharFont editor with default Color font
    pub fn new(options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        let charset_state = CharSetEditState::new();
        Self::with_charset_state(charset_state, options, font_library)
    }

    /// Create a new empty CharFont editor with the specified font type
    pub fn new_with_font_type(font_type: TdfFontType, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        let charset_state = CharSetEditState::new_with_font_type(font_type);
        Self::with_charset_state(charset_state, options, font_library)
    }

    /// Create a CharFont editor from CharSetEditState
    fn with_charset_state(charset_state: CharSetEditState, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        // Create edit buffer for the character
        let mut buffer = TextBuffer::create((30, 12));

        // Load the TDF font for rendering
        let font = BitFont::from_bytes("TDF_FONT", include_bytes!("TDF_FONT.psf")).unwrap();
        buffer.set_font(0, font);

        set_up_buffer(&mut buffer);

        let palette = buffer.palette.clone();

        // Determine the correct tool slots based on the initial font type
        let is_outline = charset_state.selected_font().map(|f| f.font_type == TdfFontType::Outline).unwrap_or(false);

        let initial_slots = if is_outline {
            tool_registry::OUTLINE_TOOL_SLOTS
        } else {
            tool_registry::CHARFONT_TOOL_SLOTS
        };

        // Create tool registry for managing tool instances with correct slots
        // Use OutlineClickTool for outline fonts
        let tool_registry = if is_outline {
            Rc::new(RefCell::new(tool_registry::ToolRegistry::new_for_outline(initial_slots, font_library.clone())))
        } else {
            Rc::new(RefCell::new(tool_registry::ToolRegistry::new(initial_slots, font_library.clone())))
        };

        // Create AnsiEditorCore with a ClickTool from the registry
        let current_tool = tool_registry.borrow_mut().take_for(tools::ToolId::Tool(icy_engine_edit::tools::Tool::Click));
        let (ansi_core, _, _) = AnsiEditorCore::from_buffer_inner(buffer, options.clone(), current_tool);

        // Create separate outline preview buffer/screen/canvas
        let mut preview_buffer = TextBuffer::create((30, 12));
        let preview_font = BitFont::from_bytes("TDF_FONT", include_bytes!("TDF_FONT.psf")).unwrap();
        preview_buffer.set_font(0, preview_font);
        set_up_buffer(&mut preview_buffer);

        let preview_edit_state = EditState::from_buffer(preview_buffer);
        let outline_preview_screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(preview_edit_state)));

        // Default outline style to 0
        {
            let mut guard = outline_preview_screen.lock();
            if let Some(state) = guard.as_any_mut().downcast_mut::<EditState>() {
                state.set_outline_style(0);
            }
        }

        let shared_monitor_settings = { options.read().monitor_settings.clone() };
        let mut outline_preview_canvas = CanvasView::new(outline_preview_screen.clone(), shared_monitor_settings);
        outline_preview_canvas.set_has_focus(false);

        let mut palette_grid = PaletteGrid::new();
        palette_grid.sync_palette(&palette, None);

        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        // Create tool panel using the registry
        let mut tool_panel = ToolPanel::new(tool_registry.clone());
        tool_panel.set_tool(ansi_core.current_tool_for_panel());

        let mut editor = Self {
            charset_state,
            ansi_core,
            tool_registry,
            color_switcher,
            palette_grid,
            tool_panel,
            font_library,
            undostack_len: 0,
            last_update_preview: 0,
            selected_outline_style: 0,
            outline_preview_screen,
            outline_preview_canvas,
        };

        editor.update_selected_char();
        editor
    }

    fn update_outline_preview(&mut self) {
        let Some(font) = self.charset_state.selected_font() else {
            return;
        };

        if font.font_type != TdfFontType::Outline {
            return;
        }

        let selected_char = self.charset_state.selected_char();
        let style = self.selected_outline_style.min(outline_style_preview::OUTLINE_STYLE_COUNT.saturating_sub(1));

        let mut guard = self.outline_preview_screen.lock();
        let Some(state) = guard.as_any_mut().downcast_mut::<EditState>() else {
            return;
        };

        state.set_outline_style(style);
        state.with_buffer_mut_no_undo(|buffer| {
            set_up_buffer(buffer);

            if let Some(ch) = selected_char {
                if let Some(glyph) = font.glyph(ch) {
                    let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                    let options = RenderOptions::default();
                    let _ = glyph.render(&mut renderer, &options);

                    // Transform TheDraw outline placeholders into actual display characters.
                    // This matches the egui behavior (outline style applied in preview).
                    let size = buffer.layers[0].size();
                    for y in 0..size.height {
                        for x in 0..size.width {
                            let mut cell = buffer.layers[0].char_at(icy_engine::Position::new(x, y));
                            let code = cell.ch as u8;

                            if code == 0xFF {
                                cell.ch = ' ';
                                buffer.layers[0].set_char((x, y), cell);
                                continue;
                            }

                            let is_placeholder = (b'A'..=b'Q').contains(&code) || code == b'@' || code == b'&';
                            if !is_placeholder {
                                continue;
                            }

                            let unicode_ch = transform_outline(style, code);
                            let cp437_ch = if let Some(&cp437) = codepages::tables::UNICODE_TO_CP437.get(&unicode_ch) {
                                char::from(cp437)
                            } else {
                                unicode_ch
                            };

                            cell.ch = cp437_ch;
                            buffer.layers[0].set_char((x, y), cell);
                        }
                    }
                }
            }
        });

        state.mark_buffer_dirty();
    }

    /// Create a CharFont editor with a file
    pub fn with_file(path: PathBuf, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> anyhow::Result<Self> {
        let charset_state = CharSetEditState::load_from_file(path)?;
        Ok(Self::with_charset_state(charset_state, options, font_library))
    }

    /// Get the file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.charset_state.file_path()
    }

    /// Set the file path (for session restore)
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.charset_state.set_file_path(Some(path));
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
    pub fn load_from_autosave(
        autosave_path: &std::path::Path,
        original_path: PathBuf,
        options: Arc<RwLock<Options>>,
        font_library: SharedFontLibrary,
    ) -> anyhow::Result<Self> {
        let data = std::fs::read(autosave_path)?;
        let fonts = load_tdf_fonts(&data)?;
        if fonts.is_empty() {
            anyhow::bail!("No fonts found in autosave file");
        }

        let mut charset_state = CharSetEditState::with_fonts(fonts, Some(original_path));
        charset_state.set_dirty(true);
        Ok(Self::with_charset_state(charset_state, options, font_library))
    }

    /// Get undo description
    pub fn undo_description(&self) -> Option<String> {
        // First check charset_state undo, then ansi_core
        self.charset_state
            .undo_description()
            .or_else(|| self.ansi_core.with_edit_state_readonly(|state| state.undo_description()))
    }

    /// Get redo description
    pub fn redo_description(&self) -> Option<String> {
        self.charset_state
            .redo_description()
            .or_else(|| self.ansi_core.with_edit_state_readonly(|state| state.redo_description()))
    }

    /// Update the tool registry and tool panel based on the current font type
    /// Outline fonts use a reduced tool set (only Click and Select) with OutlineClickTool
    fn update_tool_registry_for_font_type(&mut self) {
        let font_type = self.charset_state.selected_font().map(|f| f.font_type).unwrap_or(TdfFontType::Color);

        let is_outline = font_type == TdfFontType::Outline;
        let needed_slots = if is_outline {
            tool_registry::OUTLINE_TOOL_SLOTS
        } else {
            tool_registry::CHARFONT_TOOL_SLOTS
        };

        // Check if we need to change the registry
        let current_slots_len = self.tool_registry.borrow().num_slots();
        if current_slots_len == needed_slots.len() {
            return; // Same configuration, no change needed
        }

        // Create new tool registry with the appropriate slots and tool type
        let new_registry = if is_outline {
            Rc::new(RefCell::new(tool_registry::ToolRegistry::new_for_outline(
                needed_slots,
                self.font_library.clone(),
            )))
        } else {
            Rc::new(RefCell::new(tool_registry::ToolRegistry::new(needed_slots, self.font_library.clone())))
        };

        // Switch to click tool from the new registry
        {
            let mut reg = new_registry.borrow_mut();
            self.ansi_core.change_tool(&mut *reg, tools::ToolId::Tool(icy_engine_edit::tools::Tool::Click));
        }

        // Create new tool panel with the new registry
        let mut new_tool_panel = ToolPanel::new(new_registry.clone());
        new_tool_panel.set_tool(self.ansi_core.current_tool_for_panel());

        // Replace registry and tool panel
        self.tool_registry = new_registry;
        self.tool_panel = new_tool_panel;
    }

    /// Save the currently edited character back to the font
    /// TODO: Implement conversion from TextBuffer back to retrofont::Glyph
    fn save_old_selected_char(&mut self) {
        let undo_len = self.ansi_core.undo_stack_len();
        if undo_len == 0 {
            return;
        }

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
        let font_type = font.font_type;

        self.ansi_core.with_edit_state(|state| {
            state.with_buffer_mut_no_undo(|buffer| set_up_buffer(buffer));
            state.set_current_layer(0);
            state.set_caret_position(icy_engine::Position::new(0, 0));

            if let Some(ch) = selected_char {
                // Use TdfBufferRenderer to render the glyph
                if let Some(glyph) = font.glyph(ch) {
                    state.with_buffer_mut_no_undo(|buffer| {
                        let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                        let options = RenderOptions::default();
                        let _ = glyph.render(&mut renderer, &options);
                    });
                }
            }

            // For Block and Outline fonts, apply the current caret attribute to all visible characters
            if font_type == TdfFontType::Block || font_type == TdfFontType::Outline {
                let attr = state.get_caret().attribute;
                state.with_buffer_mut_no_undo(|buffer| {
                    apply_attribute_to_layer(&mut buffer.layers[0], attr);
                });
            }

            // Mark buffer as dirty to trigger re-render
            state.mark_buffer_dirty();
            state.get_undo_stack().lock().unwrap().clear();
        });

        self.update_outline_preview();
    }

    /// Apply current caret colors to all visible characters (for Block/Outline fonts)
    /// Uses interior mutability so it can be called from view()
    fn apply_font_type_colors(&self) {
        let font = match self.charset_state.selected_font() {
            Some(f) => f,
            None => return,
        };

        let font_type = font.font_type;

        // Only apply for Block and Outline fonts
        if font_type != TdfFontType::Block && font_type != TdfFontType::Outline {
            return;
        }

        self.ansi_core.with_edit_state_mut_shared(|state| {
            let attr = state.get_caret().attribute;
            state.with_buffer_mut_no_undo(|buffer| {
                if !buffer.layers.is_empty() {
                    apply_attribute_to_layer(&mut buffer.layers[0], attr);
                }
            });
            state.mark_buffer_dirty();
        });
    }

    /// Update the editor state
    pub fn update(&mut self, message: CharFontEditorMessage, dialogs: &mut DialogStack<Message>) -> Task<CharFontEditorMessage> {
        let task = match message {
            // ═══════════════════════════════════════════════════════════════
            // Dialog-related messages (moved from MainWindow)
            // ═══════════════════════════════════════════════════════════════
            CharFontEditorMessage::OpenTdfFontSelector => {
                // Open TDF font selector dialog
                // TODO: Get proper font library from CharFontEditor
                dialogs.push(TdfFontSelectorDialog::new(Default::default()));
                Task::none()
            }
            CharFontEditorMessage::TdfFontSelector(_) => {
                // Handled by DialogStack
                Task::none()
            }

            CharFontEditorMessage::ColorSwitcher(msg) => {
                let nested_task = match msg {
                    ColorSwitcherMessage::SwapColors => {
                        self.color_switcher.start_swap_animation();
                        Task::none()
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        let (fg, bg) = self.ansi_core.with_edit_state(|state| state.swap_caret_colors());
                        self.palette_grid.set_foreground(fg);
                        self.palette_grid.set_background(bg);
                        self.color_switcher.confirm_swap();
                        // Apply colors to Block/Outline fonts
                        self.apply_font_type_colors();
                        Task::none()
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        self.ansi_core.with_edit_state(|state| {
                            state.reset_caret_colors();
                        });
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                        // Apply colors to Block/Outline fonts
                        self.apply_font_type_colors();
                        Task::none()
                    }
                    ColorSwitcherMessage::Tick(delta) => {
                        if self.color_switcher.tick(delta) {
                            Task::done(CharFontEditorMessage::ColorSwitcher(ColorSwitcherMessage::AnimationComplete))
                        } else {
                            Task::none()
                        }
                    }
                };
                nested_task
            }
            CharFontEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        self.ansi_core.with_edit_state(|state| {
                            state.set_caret_foreground(color);
                        });
                        self.palette_grid.set_foreground(color);
                        // Apply colors to Block/Outline fonts
                        self.apply_font_type_colors();
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        self.ansi_core.with_edit_state(|state| {
                            state.set_caret_background(color);
                        });
                        self.palette_grid.set_background(color);
                        // Apply colors to Block/Outline fonts
                        self.apply_font_type_colors();
                    }
                }
                Task::none()
            }

            CharFontEditorMessage::OutlinePreviewCanvas(msg) => {
                let _ = self.outline_preview_canvas.update(msg);
                Task::none()
            }
            CharFontEditorMessage::ToolPanel(msg) => {
                // Keep tool panel internal animation state in sync.
                let _ = self.tool_panel.update(msg.clone());

                if let ToolPanelMessage::ClickSlot(slot) = msg {
                    let current_tool = self.ansi_core.current_tool_for_panel();
                    let new_tool = self.tool_registry.borrow().click_tool_slot(slot, current_tool);
                    {
                        let mut reg = self.tool_registry.borrow_mut();
                        self.ansi_core.change_tool(&mut *reg, tools::ToolId::Tool(new_tool));
                    }
                    // Tool changes may be blocked, so always sync from core.
                    self.tool_panel.set_tool(self.ansi_core.current_tool_for_panel());
                }

                Task::none()
            }
            CharFontEditorMessage::SelectFont(idx) => {
                self.save_old_selected_char();
                self.charset_state.select_font(idx);
                self.update_tool_registry_for_font_type();
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
                let u = self.ansi_core.undo_stack_len();
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
                            Task::done(CharFontEditorMessage::ExtendCharsetSelection(dx, dy, is_rectangle))
                        } else {
                            Task::done(CharFontEditorMessage::MoveCharsetCursor(dx, dy))
                        }
                    }
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit cursor movement
                        Task::none()
                    }
                }
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
                    CharSetFocusedPanel::CharSet => Task::done(CharFontEditorMessage::SelectCharAtCursor),
                    CharSetFocusedPanel::Edit => {
                        // TODO: Handle edit confirm
                        Task::none()
                    }
                }
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

            // ═══════════════════════════════════════════════════════════════
            // ANSI Editor Core Messages (for editing TDF glyphs)
            // Forwarded from MainWindow when AnsiEditorMessage::Core is received
            // in CharFont mode
            // ═══════════════════════════════════════════════════════════════
            CharFontEditorMessage::AnsiEditor(msg) => {
                // ToggleColor needs special handling for color switcher animation
                if let AnsiEditorCoreMessage::ToggleColor = &msg {
                    self.toggle_color();
                    Task::none()
                } else {
                    // Forward all other messages to the AnsiEditorCore
                    self.ansi_core.update(msg).map(CharFontEditorMessage::AnsiEditor)
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Outline Style Selection
            // ═══════════════════════════════════════════════════════════════
            CharFontEditorMessage::SelectOutlineStyle(style) => {
                self.selected_outline_style = style.min(outline_style_preview::OUTLINE_STYLE_COUNT - 1);
                self.update_outline_preview();
                Task::none()
            }
        };

        // Keep the outline preview in sync on every update (matches old egui behavior)
        self.update_outline_preview();

        task
    }

    /// Handle top-level window/input events that must reach the editor.
    /// Forwards to AnsiEditorCore for tool and keyboard handling.
    /// Returns `true` if the event was handled.
    pub fn handle_event(&mut self, event: &iced::Event) -> bool {
        self.ansi_core.handle_event(event)
    }

    /// Render the editor view
    pub fn view(&self) -> Element<'_, CharFontEditorMessage> {
        // Apply current caret colors for Block/Outline fonts before rendering
        self.apply_font_type_colors();

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

        // === LEFT SIDEBAR: Palette + Tool Panel ===
        let sidebar_width = 64.0;

        // Get caret colors for color switcher and palette
        let (caret_fg, caret_bg, palette) = self.ansi_core.with_edit_state_readonly(|state| {
            let caret = state.get_caret();
            let buffer = state.get_buffer();
            (caret.attribute.foreground(), caret.attribute.background(), buffer.palette.clone())
        });

        let palette_view = self.palette_grid.view_with_width(sidebar_width, None).map(CharFontEditorMessage::PaletteGrid);
        let bg_weakest = main_area_background(&Theme::Dark);
        let tool_panel = self
            .tool_panel
            .view_with_config(sidebar_width, bg_weakest)
            .map(CharFontEditorMessage::ToolPanel);

        let left_sidebar = column![palette_view, tool_panel].spacing(4);

        // === TOP BAR (like ANSI editor): Color switcher + tool toolbar content ===
        let color_switcher = self.color_switcher.view(caret_fg, caret_bg).map(CharFontEditorMessage::ColorSwitcher);

        // Tool-specific controls (e.g. OutlineClickTool cheat sheet)
        // This is the toolbar provided by the currently active tool (e.g. OutlineClickTool cheat sheet).
        let fkeys = self.ansi_core.options.read().fkeys.clone();
        let current_font = self.ansi_core.with_edit_state_readonly(|state| {
            let buffer = state.get_buffer();
            let caret = state.get_caret();
            let font_page = caret.font_page();
            buffer.font(font_page).or_else(|| buffer.font(0)).cloned()
        });

        let view_ctx = tools::ToolViewContext {
            theme: Theme::Dark,
            fkeys,
            font: current_font,
            palette: palette.clone(),
            caret_fg,
            caret_bg,
            tag_add_mode: false,
            selected_tag: None,
            tag_selection_count: 0,
        };

        let tool_toolbar = self
            .ansi_core
            .view_current_tool_toolbar(&view_ctx)
            .map(|m| CharFontEditorMessage::AnsiEditor(AnsiEditorCoreMessage::ToolMessage(m)));

        let top_toolbar = row![color_switcher, tool_toolbar].spacing(4).align_y(Alignment::Start);

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        // Check if current font is an Outline font - if so, show the right-side preview panel
        let is_outline_font = self.charset_state.selected_font().is_some_and(|f| f.font_type == TdfFontType::Outline);

        let outline_panel: Option<Element<'_, CharFontEditorMessage>> = if is_outline_font {
            let preview_canvas: Element<'_, CharFontEditorMessage> =
                container(self.outline_preview_canvas.view().map(CharFontEditorMessage::OutlinePreviewCanvas))
                    .style(container::bordered_box)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();

            let style_selector = outline_style_preview::view_style_selector(self.selected_outline_style, |m| match m {
                outline_style_preview::OutlineStyleSelectorMessage::Select(style) => CharFontEditorMessage::SelectOutlineStyle(style),
            });

            // Keep the selector from stealing vertical space: make it scrollable with a compact fixed height.
            let selector_height = 220.0;
            let selector_container: Element<'_, CharFontEditorMessage> =
                container(scrollable(style_selector).width(Length::Fill).height(Length::Fixed(selector_height)))
                    .style(container::bordered_box)
                    .height(Length::Fixed(selector_height))
                    .into();

            let panel_width = outline_style_preview::selector_width();

            Some(
                container(column![preview_canvas, selector_container,].spacing(6).height(Length::Fill))
                    .width(Length::Fixed(panel_width))
                    .height(Length::Fill)
                    .into(),
            )
        } else {
            None
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

        let charset_section: container::Container<'_, CharFontEditorMessage> = container(charset_canvas).style(container::bordered_box).center_x(Length::Fill);

        // === CENTER AREA: Editor canvas + optional style preview on right ===
        // Create the ansi editor view (fill available width so no dead space remains after the preview)
        let ansi_editor_view: Element<'_, CharFontEditorMessage> = container(self.ansi_core.view().map(CharFontEditorMessage::AnsiEditor))
            .width(Length::Fill)
            .into();

        // For outline fonts, place the preview panel to the right of the editor
        let editor_area: Element<'_, CharFontEditorMessage> = if let Some(panel) = outline_panel {
            row![ansi_editor_view, panel,].spacing(4).into()
        } else {
            ansi_editor_view
        };

        let center_area = column![editor_area, charset_section,].spacing(8);

        // === MAIN LAYOUT ===
        // Left: palette + tools | Center: toolbar on top, then content
        let left_content_row = row![container(left_sidebar).width(Length::Fixed(sidebar_width)), center_area,];

        // Left column: ANSI-like top toolbar, then content
        let left_column: Element<'_, CharFontEditorMessage> = column![
            container(top_toolbar)
                .width(Length::Fill)
                .height(Length::Fixed(toolbar_height))
                .style(container::rounded_box),
            left_content_row,
        ]
        .spacing(0)
        .into();

        let content = row![left_column, container(right_panel).style(container::bordered_box),].spacing(4);

        let main_layout = container(content).padding(4).width(Length::Fill).height(Length::Fill).into();

        // Wrap with modal overlays (char selector, etc.)
        self.ansi_core.wrap_with_modals_mapped(main_layout, CharFontEditorMessage::AnsiEditor)
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

    // ═══════════════════════════════════════════════════════════════════════════
    // Color operations (special handling for color switcher animation)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Toggle (swap) foreground and background colors with animation
    fn toggle_color(&mut self) {
        self.color_switcher.start_swap_animation();
        self.ansi_core.with_edit_state(|state| {
            state.swap_caret_colors();
        });
        // Sync palette grid after swap
        let (fg, bg) = self.ansi_core.with_edit_state(|state| {
            let caret = state.get_caret();
            (caret.attribute.foreground(), caret.attribute.background())
        });
        self.palette_grid.set_foreground(fg);
        self.palette_grid.set_background(bg);
        self.color_switcher.confirm_swap();
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

/// Apply a text attribute to all visible characters in a layer.
/// Used for Block and Outline fonts where all characters share the same color.
fn apply_attribute_to_layer(layer: &mut Layer, attr: TextAttribute) {
    let size = layer.size();
    for y in 0..size.height {
        for x in 0..size.width {
            let mut ch = layer.char_at(icy_engine::Position::new(x, y));
            if !ch.is_visible() {
                continue;
            }
            ch.attribute = attr;
            layer.set_char((x, y), ch);
        }
    }
}
