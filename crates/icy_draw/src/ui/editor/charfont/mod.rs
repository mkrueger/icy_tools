//! CharFont (TDF) Editor Mode
//!
//! This module contains the TDF font editor with:
//! - Left sidebar: Font list, character selector
//! - Top bar: Font name, spacing, font type info
//! - Center: Split view - top: character preview, bottom: charset grid
//! - Color switcher and ANSI toolbar for editing

mod charset_canvas;
mod font_dialogs;
mod outline_style_preview;

pub use charset_canvas::*;
pub use font_dialogs::*;

use std::path::PathBuf;
use std::sync::Arc;

use icy_engine::char_set::TdfBufferRenderer;
use icy_engine::Screen;
use icy_engine::{AttributedChar, BitFont, Layer, Size, TextAttribute, TextBuffer, TextPane};
use icy_engine_edit::charset::{load_tdf_fonts, CharSetEditState, CharSetFocusedPanel, TdfFontType};
use icy_engine_edit::EditState;
use icy_engine_edit::UndoState;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::ui::{add_icon, arrow_downward_icon, arrow_upward_icon, content_copy_icon, delete_icon, edit_icon, DialogStack};
use icy_engine_gui::TerminalMessage;
use icy_ui::{
    keyboard::Modifiers,
    widget::{button, canvas, column, container, row, scrollable, text},
    Alignment, Element, Length, Task, Theme,
};
use parking_lot::{Mutex, RwLock};
use retrofont::{transform_outline, Glyph, GlyphPart, RenderOptions};

use crate::ui::editor::ansi::constants;
use crate::ui::editor::ansi::widget::canvas::CanvasView;
use crate::ui::editor::ansi::{
    tool_registry, tools, AnsiEditorCore, AnsiEditorCoreMessage, ColorSwitcher, ColorSwitcherMessage, PaletteGrid, PaletteGridMessage, TdfFontSelectorDialog,
    ToolPanel, ToolPanelMessage,
};
use crate::ui::main_window::Message;
use crate::Settings;
use crate::SharedFontLibrary;

/// Direction for arrow key navigation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Messages for the CharFont editor
#[derive(Clone)]
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
    /// Move font up in the list
    MoveFontUp,
    /// Move font down in the list
    MoveFontDown,
    /// Open add font dialog
    OpenAddFontDialog,
    /// Open edit font settings dialog
    OpenEditSettingsDialog,
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

    // ═══════════════════════════════════════════════════════════════════════════
    // Font Dialogs
    // ═══════════════════════════════════════════════════════════════════════════
    /// Add font dialog messages
    AddFontDialog(AddFontDialogMessage),
    /// Apply the add font dialog result
    AddFontApply(TdfFontType, String, i32),
    /// Edit font settings dialog messages
    EditFontSettingsDialog(EditFontSettingsDialogMessage),
    /// Apply the edit font settings dialog result
    EditFontSettingsApply(String, i32),

    // ═══════════════════════════════════════════════════════════════════════════
    // Import/Export
    // ═══════════════════════════════════════════════════════════════════════════
    /// Import fonts from a TDF file (add to current file)
    ImportFonts,
    /// Import fonts completed (number of fonts added)
    ImportFontsComplete(usize),
    /// Import fonts with the loaded file data (raw bytes)
    ImportFontsData(Vec<u8>),
    /// Export current font as single TDF file
    ExportFont,
    /// No-op message
    Nop,
}

impl std::fmt::Debug for CharFontEditorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImportFontsData(data) => f.debug_tuple("ImportFontsData").field(&data.len()).finish(),
            Self::ColorSwitcher(_) => f.write_str("ColorSwitcher(..)"),
            Self::PaletteGrid(_) => f.write_str("PaletteGrid(..)"),
            Self::ToolPanel(_) => f.write_str("ToolPanel(..)"),
            Self::SelectFont(i) => f.debug_tuple("SelectFont").field(i).finish(),
            Self::SelectChar(c) => f.debug_tuple("SelectChar").field(c).finish(),
            Self::SelectCharAt(c, x, y) => f.debug_tuple("SelectCharAt").field(c).field(x).field(y).finish(),
            Self::CloneFont => f.write_str("CloneFont"),
            Self::DeleteFont => f.write_str("DeleteFont"),
            Self::MoveFontUp => f.write_str("MoveFontUp"),
            Self::MoveFontDown => f.write_str("MoveFontDown"),
            Self::OpenAddFontDialog => f.write_str("OpenAddFontDialog"),
            Self::OpenEditSettingsDialog => f.write_str("OpenEditSettingsDialog"),
            Self::ClearChar => f.write_str("ClearChar"),
            Self::FontNameChanged(_) => f.write_str("FontNameChanged(..)"),
            Self::SpacingChanged(s) => f.debug_tuple("SpacingChanged").field(s).finish(),
            Self::Tick(_) => f.write_str("Tick(..)"),
            Self::MoveCharsetCursor(x, y) => f.debug_tuple("MoveCharsetCursor").field(x).field(y).finish(),
            Self::SetCharsetCursor(x, y) => f.debug_tuple("SetCharsetCursor").field(x).field(y).finish(),
            Self::ExtendCharsetSelection(x, y, r) => f.debug_tuple("ExtendCharsetSelection").field(x).field(y).field(r).finish(),
            Self::SetCharsetSelectionLead(x, y, r) => f.debug_tuple("SetCharsetSelectionLead").field(x).field(y).field(r).finish(),
            Self::ClearCharsetSelection => f.write_str("ClearCharsetSelection"),
            Self::SelectCharAtCursor => f.write_str("SelectCharAtCursor"),
            Self::FocusNextPanel => f.write_str("FocusNextPanel"),
            Self::SetFocusedPanel(p) => f.debug_tuple("SetFocusedPanel").field(p).finish(),
            Self::HandleArrow(d, _) => f.debug_tuple("HandleArrow").field(d).finish(),
            Self::HandleHome => f.write_str("HandleHome"),
            Self::HandleEnd => f.write_str("HandleEnd"),
            Self::HandlePageUp => f.write_str("HandlePageUp"),
            Self::HandlePageDown => f.write_str("HandlePageDown"),
            Self::HandleConfirm => f.write_str("HandleConfirm"),
            Self::HandleCancel => f.write_str("HandleCancel"),
            Self::HandleDelete => f.write_str("HandleDelete"),
            Self::OpenTdfFontSelector => f.write_str("OpenTdfFontSelector"),
            Self::TdfFontSelector(_) => f.write_str("TdfFontSelector(..)"),
            Self::AnsiEditor(_) => f.write_str("AnsiEditor(..)"),
            Self::SelectOutlineStyle(s) => f.debug_tuple("SelectOutlineStyle").field(s).finish(),
            Self::OutlinePreviewCanvas(_) => f.write_str("OutlinePreviewCanvas(..)"),
            Self::AddFontDialog(_) => f.write_str("AddFontDialog(..)"),
            Self::AddFontApply(t, n, s) => f.debug_tuple("AddFontApply").field(t).field(n).field(s).finish(),
            Self::EditFontSettingsDialog(_) => f.write_str("EditFontSettingsDialog(..)"),
            Self::EditFontSettingsApply(n, s) => f.debug_tuple("EditFontSettingsApply").field(n).field(s).finish(),
            Self::ImportFonts => f.write_str("ImportFonts"),
            Self::ImportFontsComplete(c) => f.debug_tuple("ImportFontsComplete").field(c).finish(),
            Self::ExportFont => f.write_str("ExportFont"),
            Self::Nop => f.write_str("Nop"),
        }
    }
}

/// The CharFont (TDF) editor component
pub struct CharFontEditor {
    /// The CharSet edit state (model layer from icy_engine_edit)
    charset_state: CharSetEditState,
    /// The ANSI editor core for editing TDF glyphs
    ansi_core: AnsiEditorCore,
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
    /// Last character loaded into the edit buffer (for save_old_selected_char)
    last_edited_char: Option<char>,

    /// Separate preview screen + canvas for outline fonts (egui-like preview buffer)
    outline_preview_screen: Arc<Mutex<Box<dyn Screen>>>,
    outline_preview_canvas: CanvasView,
}

impl CharFontEditor {
    /// Create a new empty CharFont editor with default Color font
    pub fn new(options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let charset_state = CharSetEditState::new();
        Self::with_charset_state(charset_state, options, font_library)
    }

    /// Create a new empty CharFont editor with the specified font type
    pub fn new_with_font_type(font_type: TdfFontType, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let charset_state = CharSetEditState::new_with_font_type(font_type);
        Self::with_charset_state(charset_state, options, font_library)
    }

    /// Create a CharFont editor from CharSetEditState
    fn with_charset_state(charset_state: CharSetEditState, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
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
        let mut tool_registry = if is_outline {
            tool_registry::ToolRegistry::new_for_outline(initial_slots, font_library.clone())
        } else {
            tool_registry::ToolRegistry::new(initial_slots, font_library.clone())
        };

        // Create AnsiEditorCore with a ClickTool from the registry
        let current_tool = tool_registry.take_for(tools::ToolId::Tool(icy_engine_edit::tools::Tool::Click));
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

        let shared_monitor_settings = Arc::new(RwLock::new(options.read().monitor_settings.clone()));
        let mut outline_preview_canvas = CanvasView::new(outline_preview_screen.clone(), shared_monitor_settings);
        outline_preview_canvas.set_has_focus(false);

        let mut palette_grid = PaletteGrid::new();
        palette_grid.sync_palette(&palette, None);

        let mut color_switcher = ColorSwitcher::new();
        color_switcher.sync_palette(&palette);

        // Create tool panel using the registry
        let mut tool_panel = ToolPanel::new(tool_registry);
        tool_panel.set_tool(ansi_core.current_tool_for_panel());

        let mut editor = Self {
            charset_state,
            ansi_core,
            color_switcher,
            palette_grid,
            tool_panel,
            font_library,
            undostack_len: 0,
            last_update_preview: 0,
            selected_outline_style: 0,
            last_edited_char: None,
            outline_preview_screen,
            outline_preview_canvas,
        };

        // Initialize focus state - CharSet starts focused by default
        editor.sync_canvas_focus();
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

        // Convert the current editor buffer to a glyph - this is the source of truth
        let glyph = self
            .ansi_core
            .with_edit_state(|state| buffer_to_glyph(state.get_buffer(), TdfFontType::Outline));

        let mut guard = self.outline_preview_screen.lock();
        let Some(state) = guard.as_any_mut().downcast_mut::<EditState>() else {
            return;
        };

        state.set_outline_style(style);
        let buffer = state.get_buffer_mut();
        set_up_buffer(buffer);

        if selected_char.is_some() {
            if let Some(glyph) = &glyph {
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

        state.mark_buffer_dirty();
    }

    /// Create a CharFont editor with a file
    pub fn with_file(path: PathBuf, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> anyhow::Result<Self> {
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

    /// Get session data for serialization
    pub fn get_session_data(&self) -> Option<crate::session::CharFontSessionState> {
        let ansi_state = self.ansi_core.get_session_data()?;
        Some(crate::session::CharFontSessionState {
            version: 1,
            ansi_state,
            selected_slot: self.charset_state.selected_char().map(|c| c as usize).unwrap_or(0),
            preview_text: String::new(), // TODO: Implement preview text retrieval
        })
    }

    /// Restore session data from serialization
    pub fn set_session_data(&mut self, state: crate::session::CharFontSessionState) {
        self.ansi_core.set_session_data(state.ansi_state);
        if let Some(ch) = char::from_u32(state.selected_slot as u32) {
            self.charset_state.select_char(ch);
        }
        // Note: preview_text is not easily settable without more refactoring
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
        options: Arc<RwLock<Settings>>,
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
        // Compare both slot count AND outline mode (different click tool implementation)
        let (current_slots_len, current_uses_outline) = (self.tool_panel.registry.num_slots(), self.tool_panel.registry.uses_outline_click());

        if current_slots_len == needed_slots.len() && current_uses_outline == is_outline {
            return; // Same configuration, no change needed
        }

        // Create new tool registry with the appropriate slots and tool type
        let mut new_registry = if is_outline {
            tool_registry::ToolRegistry::new_for_outline(needed_slots, self.font_library.clone())
        } else {
            tool_registry::ToolRegistry::new(needed_slots, self.font_library.clone())
        };

        // Switch to click tool from the new registry (force because tool ID might be same but implementation differs)
        {
            self.ansi_core
                .force_change_tool(&mut new_registry, tools::ToolId::Tool(icy_engine_edit::tools::Tool::Click));
        }

        // Create new tool panel with the new registry
        let mut new_tool_panel = ToolPanel::new(new_registry);
        new_tool_panel.set_tool(self.ansi_core.current_tool_for_panel());

        // Replace tool panel (it owns the registry)
        self.tool_panel = new_tool_panel;
    }

    /// Save the currently edited character back to the font
    /// Converts the TextBuffer back to a retrofont::Glyph based on font type
    fn save_old_selected_char(&mut self) {
        let undo_len = self.ansi_core.undo_stack_len();
        if undo_len == 0 {
            return;
        }

        // Get the character that was loaded into the buffer (not the currently selected one)
        let Some(edited_char) = self.last_edited_char else {
            return;
        };

        let Some(font) = self.charset_state.selected_font() else {
            return;
        };

        let font_type = font.font_type;

        // Convert the buffer back to a Glyph
        let glyph = self.ansi_core.with_edit_state(|state| buffer_to_glyph(state.get_buffer(), font_type));

        if let Some(glyph) = glyph {
            self.charset_state.set_glyph(edited_char, glyph);
        } else {
            // Empty glyph - remove it from the font
            self.charset_state.clear_glyph(edited_char);
        }

        self.undostack_len += 1;
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
            set_up_buffer(state.get_buffer_mut());
            state.set_current_layer(0);
            state.set_caret_position(icy_engine::Position::new(0, 0));

            if let Some(ch) = selected_char {
                // Use TdfBufferRenderer to render the glyph
                if let Some(glyph) = font.glyph(ch) {
                    let buffer = state.get_buffer_mut();
                    let mut renderer = TdfBufferRenderer::new(buffer, 0, 0);
                    let options = RenderOptions {
                        render_mode: retrofont::RenderMode::Edit,
                        outline_style: 0,
                    };
                    let _ = glyph.render(&mut renderer, &options);
                }
            }

            // For Block and Outline fonts, apply the current caret attribute to all visible characters
            if font_type == TdfFontType::Block || font_type == TdfFontType::Outline {
                let attr = state.get_caret().attribute;
                apply_attribute_to_layer(&mut state.get_buffer_mut().layers[0], attr);
            }

            // Mark buffer as dirty to trigger re-render
            state.mark_buffer_dirty();
            state.get_undo_stack().lock().unwrap().clear();
        });

        // Remember which character we loaded into the buffer for save_old_selected_char
        self.last_edited_char = selected_char;

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
            let buffer = state.get_buffer_mut();
            if !buffer.layers.is_empty() {
                apply_attribute_to_layer(&mut buffer.layers[0], attr);
            }
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
                dialogs.push(TdfFontSelectorDialog::new(Default::default(), 0));
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
                    let new_tool = { self.tool_panel.registry.click_tool_slot(slot, current_tool) };
                    self.ansi_core.change_tool(&mut self.tool_panel.registry, tools::ToolId::Tool(new_tool));
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
                // Set focus to CharSet when selecting a char
                self.charset_state.set_focused_panel(CharSetFocusedPanel::CharSet);
                self.sync_canvas_focus();
                Task::none()
            }
            CharFontEditorMessage::SelectCharAt(_ch, col, row) => {
                self.charset_state.select_char_at(col, row);
                self.update_selected_char();
                // Set focus to CharSet when clicking on charset grid
                self.charset_state.set_focused_panel(CharSetFocusedPanel::CharSet);
                self.sync_canvas_focus();
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
            CharFontEditorMessage::MoveFontUp => {
                self.charset_state.move_font_up();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::MoveFontDown => {
                self.charset_state.move_font_down();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::OpenAddFontDialog => {
                dialogs.push(AddFontDialog::new());
                Task::none()
            }
            CharFontEditorMessage::OpenEditSettingsDialog => {
                if let Some(font) = self.charset_state.selected_font() {
                    dialogs.push(EditFontSettingsDialog::new(font.name.clone(), font.spacing));
                }
                Task::none()
            }
            CharFontEditorMessage::AddFontDialog(_) => {
                // Handled by DialogStack
                Task::none()
            }
            CharFontEditorMessage::AddFontApply(font_type, name, spacing) => {
                self.charset_state.add_font(font_type, name, spacing);
                self.update_selected_char();
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::EditFontSettingsDialog(_) => {
                // Handled by DialogStack
                Task::none()
            }
            CharFontEditorMessage::EditFontSettingsApply(name, spacing) => {
                self.charset_state.set_font_name(name);
                self.charset_state.set_font_spacing(spacing);
                self.undostack_len += 1;
                Task::none()
            }
            CharFontEditorMessage::ImportFonts => Task::perform(
                async move {
                    if let Some(handle) = rfd::AsyncFileDialog::new().add_filter("TDF Font", &["tdf"]).pick_file().await {
                        handle.read().await
                    } else {
                        Vec::new()
                    }
                },
                |data| {
                    if data.is_empty() {
                        CharFontEditorMessage::Nop
                    } else {
                        CharFontEditorMessage::ImportFontsData(data)
                    }
                },
            ),
            CharFontEditorMessage::ImportFontsData(data) => {
                if let Ok(fonts) = icy_engine_edit::charset::load_tdf_fonts(&data) {
                    let count = fonts.len();
                    for font in fonts {
                        self.charset_state.fonts_mut().push(font);
                    }
                    self.undostack_len += 1;
                    self.update_selected_char();
                    Task::done(CharFontEditorMessage::ImportFontsComplete(count))
                } else {
                    Task::none()
                }
            }
            CharFontEditorMessage::ImportFontsComplete(_count) => {
                // Import completed successfully - nothing else to do
                Task::none()
            }
            CharFontEditorMessage::ExportFont => {
                if let Some(font) = self.charset_state.selected_font() {
                    let font_clone = font.clone();
                    let default_name = format!("{}.tdf", font.name);
                    Task::perform(
                        async move {
                            rfd::AsyncFileDialog::new()
                                .set_file_name(&default_name)
                                .add_filter("TDF Font", &["tdf"])
                                .save_file()
                                .await
                                .map(|h| h.path().to_path_buf())
                        },
                        move |path| {
                            if let Some(path) = path {
                                if let Ok(data) = font_clone.to_bytes() {
                                    let _ = std::fs::write(&path, data);
                                }
                            }
                            CharFontEditorMessage::Nop
                        },
                    )
                } else {
                    Task::none()
                }
            }
            CharFontEditorMessage::Nop => Task::none(),
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
                self.sync_canvas_focus();
                Task::none()
            }
            CharFontEditorMessage::SetFocusedPanel(panel) => {
                self.charset_state.set_focused_panel(panel);
                self.sync_canvas_focus();
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
                // Any interaction with the ANSI editor sets focus to Edit panel
                if self.charset_state.focused_panel() != CharSetFocusedPanel::Edit {
                    self.charset_state.set_focused_panel(CharSetFocusedPanel::Edit);
                    self.sync_canvas_focus();
                }

                // Forward all other messages to the AnsiEditorCore
                self.ansi_core.update(msg).map(CharFontEditorMessage::AnsiEditor)
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

    /// Sync the canvas focus state with the current focused panel
    fn sync_canvas_focus(&mut self) {
        let has_edit_focus = self.charset_state.focused_panel() == CharSetFocusedPanel::Edit;
        self.ansi_core.canvas.set_has_focus(has_edit_focus);
    }

    /// Handle top-level window/input events that must reach the editor.
    /// Tab switches focus between Edit and CharSet panels.
    /// Other events are forwarded to AnsiEditorCore only when the Edit panel has focus.
    /// Returns `true` if the event was handled.
    pub fn handle_event(&mut self, event: &icy_ui::Event) -> bool {
        // Handle Tab key to switch focus between panels
        if let icy_ui::Event::Keyboard(icy_ui::keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
            if *key == icy_ui::keyboard::Key::Named(icy_ui::keyboard::key::Named::Tab) && !modifiers.command() {
                self.charset_state.focus_next_panel();
                self.sync_canvas_focus();
                return true;
            }
        }

        // Only forward events to AnsiEditorCore when the Edit panel has focus
        if self.charset_state.focused_panel() == CharSetFocusedPanel::Edit {
            self.ansi_core.handle_event(event)
        } else {
            false
        }
    }

    /// Render the editor view
    ///
    /// The optional `chat_panel` parameter is accepted for API consistency but
    /// currently ignored since charfont editor doesn't support collaboration.
    pub fn view(&self, _chat_panel: Option<Element<'_, CharFontEditorMessage>>) -> Element<'_, CharFontEditorMessage> {
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

        // Icon size for buttons
        let icon_size = 16.0;
        let selected_idx = self.charset_state.selected_font_index();
        let font_count = self.charset_state.font_count();

        // Font actions - first row: delete, clone, add, settings
        let font_actions_row1 = row![
            button(delete_icon(icon_size))
                .on_press_maybe(if font_count > 1 { Some(CharFontEditorMessage::DeleteFont) } else { None })
                .padding(4),
            button(content_copy_icon(icon_size)).on_press(CharFontEditorMessage::CloneFont).padding(4),
            button(add_icon(icon_size)).on_press(CharFontEditorMessage::OpenAddFontDialog).padding(4),
            button(edit_icon(icon_size)).on_press(CharFontEditorMessage::OpenEditSettingsDialog).padding(4),
        ]
        .spacing(2);

        // Font actions - second row: move up, move down
        let font_actions_row2 = row![
            button(arrow_upward_icon(icon_size))
                .on_press_maybe(if selected_idx > 0 { Some(CharFontEditorMessage::MoveFontUp) } else { None })
                .padding(4),
            button(arrow_downward_icon(icon_size))
                .on_press_maybe(if selected_idx + 1 < font_count {
                    Some(CharFontEditorMessage::MoveFontDown)
                } else {
                    None
                })
                .padding(4),
        ]
        .spacing(2);

        let font_actions = column![font_actions_row1, font_actions_row2].spacing(2);

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
        let bg_weakest = main_area_background(&Theme::dark());
        let icon_color = Theme::dark().background.on;
        let tool_panel = self
            .tool_panel
            .view_with_config(sidebar_width, bg_weakest, icon_color)
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
            theme: Theme::dark(),
            fkeys,
            font: current_font,
            palette: palette.clone(),
            caret_fg,
            caret_bg,
            tag_add_mode: false,
            selected_tag: None,
            tag_selection_count: 0,
            is_image_layer: false,
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
                container(self.outline_preview_canvas.view(None).map(CharFontEditorMessage::OutlinePreviewCanvas))
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
            .map(|(a, l, r)| (icy_ui::Point::new(a.x, a.y), icy_ui::Point::new(l.x, l.y), r));

        let charset_is_focused = self.charset_state.focused_panel() == CharSetFocusedPanel::CharSet;
        let edit_is_focused = self.charset_state.focused_panel() == CharSetFocusedPanel::Edit;

        let charset_canvas = canvas::Canvas::new(CharSetCanvas {
            font: font_ref,
            selected_char: self.charset_state.selected_char(),
            cursor_col,
            cursor_row,
            is_focused: charset_is_focused,
            selection,
            cell_width,
            cell_height,
            label_size,
        })
        .width(Length::Fixed(canvas_width))
        .height(Length::Fixed(canvas_height));

        // Focused container style with highlight border
        let charset_container_style = move |theme: &icy_ui::Theme| {
            let border_color = if charset_is_focused { theme.accent.base } else { theme.primary.divider };
            container::Style {
                border: icy_ui::Border {
                    color: border_color,
                    width: if charset_is_focused { 2.0 } else { 1.0 },
                    radius: 4.0.into(),
                },
                ..container::bordered_box(theme)
            }
        };

        let charset_section: container::Container<'_, CharFontEditorMessage> = container(charset_canvas).style(charset_container_style).center_x(Length::Fill);

        // === CENTER AREA: Editor canvas + optional style preview on right ===
        // Focused container style for ANSI editor
        let ansi_container_style = move |theme: &icy_ui::Theme| {
            let border_color = if edit_is_focused { theme.accent.base } else { theme.primary.divider };
            container::Style {
                border: icy_ui::Border {
                    color: border_color,
                    width: if edit_is_focused { 2.0 } else { 1.0 },
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        };

        // Create the ansi editor view (fill available width so no dead space remains after the preview)
        let ansi_editor_view: Element<'_, CharFontEditorMessage> = container(self.ansi_core.view().map(CharFontEditorMessage::AnsiEditor))
            .width(Length::Fill)
            .style(ansi_container_style)
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

/// Convert a TextBuffer back to a retrofont::Glyph
///
/// This function extracts the glyph content from the edit buffer and converts it
/// back to the appropriate GlyphPart representation based on the font type:
///
/// - **Color**: Each cell becomes `GlyphPart::AnsiChar { ch, fg, bg, blink }`
/// - **Block**: Each cell becomes `GlyphPart::Char(ch)` or `GlyphPart::HardBlank` for 0xFF
/// - **Outline**: Cells are converted back to outline markers (A-R, @, O) or `GlyphPart::Char`
fn buffer_to_glyph(buffer: &TextBuffer, font_type: TdfFontType) -> Option<Glyph> {
    if buffer.layers.is_empty() {
        return None;
    }

    let layer = &buffer.layers[0];
    let size = layer.size();

    // First, determine the actual bounding box of the content
    // (skip trailing empty rows and columns)
    let mut max_x: i32 = 0;
    let mut max_y: i32 = 0;

    for y in 0..size.height {
        for x in 0..size.width {
            let cell = layer.char_at(icy_engine::Position::new(x, y));
            if !is_empty_cell(cell.ch) {
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
            }
        }
    }

    // Empty glyph - return None to remove it
    if max_x == 0 || max_y == 0 {
        return None;
    }

    let width = max_x as usize;
    let height = max_y as usize;

    let mut parts = Vec::new();

    for y in 0..height as i32 {
        // Skip trailing empty cells on each line for efficiency
        let mut line_width = 0i32;
        for x in (0..width as i32).rev() {
            let cell = layer.char_at(icy_engine::Position::new(x, y));
            if !is_empty_cell(cell.ch) {
                line_width = x + 1;
                break;
            }
        }

        for x in 0..line_width {
            let cell = layer.char_at(icy_engine::Position::new(x, y));
            let part = convert_cell_to_glyph_part(cell, font_type);
            parts.push(part);
        }

        // Add NewLine between lines (but not after the last line)
        if y < height as i32 - 1 {
            parts.push(GlyphPart::NewLine);
        }
    }

    Some(Glyph { width, height, parts })
}

/// Check if a cell is effectively empty (space or null)
fn is_empty_cell(ch: char) -> bool {
    ch == ' ' || ch == '\0'
}

/// Convert a single AttributedChar cell to the appropriate GlyphPart based on font type
fn convert_cell_to_glyph_part(cell: AttributedChar, font_type: TdfFontType) -> GlyphPart {
    let ch = cell.ch;
    let attr = cell.attribute;

    match font_type {
        TdfFontType::Color => {
            // Color fonts use AnsiChar with full color information
            let fg = attr.foreground() as u8;
            let bg = attr.background() as u8;
            let blink = attr.is_blinking();

            // Handle hard blank (0xFF in CP437 = NBSP in Unicode)
            if ch == '\u{00A0}' || ch as u32 == 0xFF {
                GlyphPart::HardBlank
            } else {
                // Convert from CP437 display char back to Unicode for storage
                let unicode_ch = cp437_to_unicode(ch);
                GlyphPart::AnsiChar { ch: unicode_ch, fg, bg, blink }
            }
        }
        TdfFontType::Block => {
            // Block fonts use simple Char without color
            // Handle hard blank (0xFF in CP437 = NBSP in Unicode)
            if ch == '\u{00A0}' || ch as u32 == 0xFF {
                GlyphPart::HardBlank
            } else {
                let unicode_ch = cp437_to_unicode(ch);
                GlyphPart::Char(unicode_ch)
            }
        }
        TdfFontType::Outline => {
            // Outline fonts use special markers for outline characters
            // The editor shows the raw placeholder characters (A-R, @, O)
            // We need to recognize and convert them back
            convert_outline_cell_to_part(ch)
        }
    }
}

/// Convert an outline font cell back to the appropriate GlyphPart
///
/// In edit mode, outline fonts show:
/// - 'A' through 'R' (or 'Q') as placeholder letters
/// - '@' as fill marker
/// - 'O' as outline hole
/// - Space as regular space
fn convert_outline_cell_to_part(ch: char) -> GlyphPart {
    let code = ch as u8;

    // Check for outline placeholder letters (A-R)
    if (b'A'..=b'R').contains(&code) {
        return GlyphPart::OutlinePlaceholder(code);
    }

    // Fill marker
    if code == b'@' {
        return GlyphPart::FillMarker;
    }

    // Outline hole
    if code == b'O' {
        // Note: 'O' could be a regular 'O' character or the outline hole marker
        // In outline fonts, 'O' at position is typically the hole marker
        return GlyphPart::OutlineHole;
    }

    // End marker
    if code == b'&' {
        return GlyphPart::EndMarker;
    }

    // Hard blank
    if ch == '\u{00A0}' || code == 0xFF {
        return GlyphPart::HardBlank;
    }

    // Regular character (including space)
    let unicode_ch = cp437_to_unicode(ch);
    GlyphPart::Char(unicode_ch)
}

/// Convert a CP437 character to Unicode
/// The buffer stores characters in CP437 format, we need to convert back to Unicode
fn cp437_to_unicode(ch: char) -> char {
    // The buffer uses a custom font and may store chars directly or in CP437
    // For characters < 256, use the mapping table
    let code = ch as u32;
    if code < 256 {
        // Use the codepages table for accurate conversion
        if let Some(&unicode) = codepages::tables::CP437_TO_UNICODE.get(code as usize) {
            return unicode;
        }
    }
    ch
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
