use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{column, container, pane_grid, row},
};
use icy_engine::TextPane;
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine_edit::EditState;
use icy_engine_edit::UndoState;
use icy_engine_edit::tools::Tool;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::ui::{DialogStack, export_dialog_with_defaults_from_msg};
use parking_lot::RwLock;

use crate::Plugin;
use crate::Settings;
use crate::SharedFontLibrary;
use crate::ui::editor::palette::PaletteEditorDialog;
use crate::ui::main_window::Message;
use crate::ui::{LayerMessage, MinimapMessage};

use crate::ui::widget::paste_controls::{PasteControls, PasteControlsMessage};

use super::*;

/// Public entrypoint for the ANSI editor mode.
///
/// Owns the core editor (`AnsiEditorCore`) privately and provides the surrounding
/// layout/panels (tool panel, palette grid, right panel, overlays).
pub struct AnsiEditorMainArea {
    core: AnsiEditorCore,
    /// File path (if saved)
    file_path: Option<PathBuf>,
    /// Tool panel state (left sidebar icons)
    tool_panel: ToolPanel,
    /// Palette grid
    palette_grid: PaletteGrid,
    /// Right panel state (minimap, layers)
    right_panel: RightPanel,

    /// Center split state (canvas + chat)
    center_panes: pane_grid::State<CenterPane>,
    /// Double-click detector for font slot buttons
    slot_double_click: RefCell<icy_engine_gui::DoubleClickDetector<usize>>,
    /// Paste controls widget
    paste_controls: PasteControls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CenterPane {
    Canvas,
    Chat,
}

impl AnsiEditorMainArea {
    pub fn new(options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let mut buffer = icy_engine::TextBuffer::new((80, 25));
        buffer.terminal_state.is_terminal_buffer = false;
        Self::with_buffer(buffer, None, options, font_library)
    }

    pub fn with_buffer(buffer: icy_engine::TextBuffer, file_path: Option<PathBuf>, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Self {
        let mut tool_registry = tool_registry::ToolRegistry::new(tool_registry::ANSI_TOOL_SLOTS, font_library);

        // Default tool is Click. Take it from the registry so it becomes the active boxed tool.
        let mut current_tool = tool_registry.take_for(tools::ToolId::Tool(Tool::Click));
        if let Some(click) = current_tool.as_any_mut().downcast_mut::<tools::ClickTool>() {
            click.sync_fkey_set_from_options(&options);
        }

        let (core, palette, format_mode) = AnsiEditorCore::from_buffer_inner(buffer, options, current_tool);

        let mut tool_panel = ToolPanel::new(tool_registry);
        tool_panel.set_tool(core.current_tool_for_panel());

        let mut palette_grid = PaletteGrid::new();
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        palette_grid.sync_palette(&palette, palette_limit);

        Self {
            core,
            file_path,
            tool_panel,
            palette_grid,
            right_panel: RightPanel::new(),
            center_panes: {
                let (mut panes, canvas_pane) = pane_grid::State::new(CenterPane::Canvas);
                let _ = panes.split(pane_grid::Axis::Horizontal, canvas_pane, CenterPane::Chat);
                panes
            },
            slot_double_click: RefCell::new(icy_engine_gui::DoubleClickDetector::new()),
            paste_controls: PasteControls::new(),
        }
    }

    pub fn with_file(path: PathBuf, options: Arc<RwLock<Settings>>, font_library: SharedFontLibrary) -> Result<Self, String> {
        let format = FileFormat::from_path(&path).unwrap_or(FileFormat::Ansi);
        let screen = format.load(&path, Some(LoadData::default())).map_err(|e| e.to_string())?;
        Ok(Self::with_buffer(screen.buffer, Some(path), options, font_library))
    }

    pub fn load_from_autosave(
        autosave_path: &Path,
        original_path: PathBuf,
        options: Arc<RwLock<Settings>>,
        font_library: SharedFontLibrary,
    ) -> Result<Self, String> {
        let data = std::fs::read(autosave_path).map_err(|e| format!("Failed to load autosave: {}", e))?;
        let format = FileFormat::from_path(&original_path).unwrap_or(FileFormat::Ansi);
        let screen = format.from_bytes(&data, Some(LoadData::default())).map_err(|e| e.to_string())?;

        let mut editor = Self::with_buffer(screen.buffer, Some(original_path), options, font_library);
        editor.core.is_modified = true;
        Ok(editor)
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    pub fn undo_stack_len(&self) -> usize {
        self.core.undo_stack_len()
    }

    /// Get a clone of the undo stack for serialization
    #[allow(dead_code)]
    pub fn get_undo_stack(&self) -> Option<icy_engine_edit::EditorUndoStack> {
        self.core.get_undo_stack()
    }

    /// Restore undo stack from serialization
    #[allow(dead_code)]
    pub fn set_undo_stack(&mut self, stack: icy_engine_edit::EditorUndoStack) {
        self.core.set_undo_stack(stack);
    }

    /// Get collaboration sync info: (undo_stack_arc, position, has_selection)
    /// Returns None if not in EditState mode
    /// When selecting, returns the selection lead position; otherwise returns caret position
    pub fn get_collab_sync_info(&self) -> Option<(std::sync::Arc<std::sync::Mutex<icy_engine_edit::EditorUndoStack>>, (i32, i32), bool)> {
        let mut screen = self.core.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_ref::<EditState>() {
            let undo_stack = state.get_undo_stack();
            // If we have a selection, use the selection lead position (for Selection events)
            // Otherwise use the caret position (for Cursor events)
            let (pos, selecting) = if let Some(sel) = state.selection() {
                ((sel.lead.x, sel.lead.y), true)
            } else {
                let caret = state.get_caret();
                ((caret.x, caret.y), false)
            };
            Some((undo_stack, pos, selecting))
        } else {
            None
        }
    }

    /// Get floating layer blocks for collaboration PasteAsSelection
    pub fn get_floating_layer_blocks(&self) -> Option<icy_engine_edit::collaboration::Blocks> {
        let mut screen = self.core.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_ref::<EditState>() {
            state.get_floating_layer_blocks()
        } else {
            None
        }
    }

    /// Get floating layer position for collaboration Operation events
    pub fn get_floating_layer_position(&self) -> Option<(i32, i32)> {
        let mut screen = self.core.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_ref::<EditState>() {
            state.get_floating_layer_position()
        } else {
            None
        }
    }

    /// Take pending collaboration events from the editor (clears the queue)
    pub fn take_pending_collab_events(&mut self) -> Vec<super::CollabToolEvent> {
        self.core.take_pending_collab_events()
    }

    /// Get session data for serialization
    pub fn get_session_data(&self) -> Option<icy_engine_edit::AnsiEditorSessionState> {
        self.core.get_session_data()
    }

    /// Restore session data from serialization
    pub fn set_session_data(&mut self, state: icy_engine_edit::AnsiEditorSessionState) {
        self.core.set_session_data(state);
    }

    /// Get the current buffer dimensions (columns, rows)
    pub fn get_buffer_dimensions(&self) -> (u32, u32) {
        use icy_engine::TextPane;
        let mut screen = self.core.screen.lock();
        if let Some(state) = screen.as_any_mut().downcast_ref::<EditState>() {
            let buffer = state.get_buffer();
            (buffer.width() as u32, buffer.height() as u32)
        } else {
            (80, 25)
        }
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.core.save(path)
    }

    /// Get bytes for autosave (saves in ICY format with thumbnail skipped for performance)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        let mut screen = self.core.screen.lock();
        if let Some(edit_state) = screen.as_any_mut().downcast_ref::<EditState>() {
            // Use ICY format for autosave to preserve all data (layers, fonts, etc.)
            let format = FileFormat::IcyDraw;
            let buffer = edit_state.get_buffer();
            // Skip thumbnail generation for faster autosave
            let mut options = icy_engine::AnsiSaveOptionsV2::default();
            options.skip_thumbnail = true;
            format.to_bytes(buffer, &options).map_err(|e| e.to_string())
        } else {
            Err("Could not access edit state".to_string())
        }
    }

    pub fn get_marker_menu_state(&self) -> widget::toolbar::menu_bar::MarkerMenuState {
        self.core.get_marker_menu_state()
    }

    pub fn get_mirror_mode(&self) -> bool {
        self.core.get_mirror_mode()
    }

    /// Get the undo description for menu display
    pub fn undo_description(&self) -> Option<String> {
        self.with_edit_state_readonly(|state| state.undo_description())
    }

    /// Get the redo description for menu display
    pub fn redo_description(&self) -> Option<String> {
        self.with_edit_state_readonly(|state| state.redo_description())
    }

    /// Get mirror mode state for menu display
    pub fn mirror_mode(&self) -> bool {
        self.get_mirror_mode()
    }

    pub fn zoom_in(&mut self) {
        self.core.canvas.zoom_in();
    }

    pub fn zoom_out(&mut self) {
        self.core.canvas.zoom_out();
    }

    pub fn zoom_reset(&mut self) {
        self.core.canvas.zoom_reset();
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.core.canvas.set_zoom(zoom);
    }

    /// Apply a remote draw operation from collaboration
    pub fn apply_remote_draw(&mut self, x: i32, y: i32, code: u32, fg: u8, bg: u8) {
        use icy_engine::TextPane;
        self.core.with_edit_state(|state| {
            let buffer = state.get_buffer_mut();
            let pos = (x, y);
            let ch = buffer.layers[0].char_at(pos.into());
            let mut new_ch = ch;
            new_ch.ch = char::from_u32(code).unwrap_or(' ');
            new_ch.attribute.set_foreground(fg as u32);
            new_ch.attribute.set_background(bg as u32);
            buffer.layers[0].set_char(pos, new_ch);
            buffer.mark_dirty();
        });
    }

    /// Apply a full remote document snapshot from collaboration.
    ///
    /// Sets all document properties: size, content, ice colors, 9px font, etc.
    pub fn apply_remote_document(&mut self, doc: &icy_engine_edit::collaboration::ConnectedDocument) {
        use icy_engine::{AttributedChar, IceMode, Position};

        let cols_i32 = doc.columns as i32;
        let rows_i32 = doc.rows as i32;

        log::info!(
            "[COLLAB] apply_remote_document: received doc with columns={}, rows={}, document.len()={}",
            doc.columns,
            doc.rows,
            doc.document.len()
        );

        self.core.with_edit_state(|state| {
            let buffer = state.get_buffer_mut();
            let before_w = buffer.width();
            let before_h = buffer.height();
            let before_lines = buffer.layers.first().map(|l| l.lines.len()).unwrap_or(0);
            log::info!(
                "[COLLAB] apply_remote_document: buffer BEFORE set_size: width={}, height={}, layer0.lines.len()={}",
                before_w,
                before_h,
                before_lines
            );

            // Set document size
            buffer.set_size((cols_i32, rows_i32));
            buffer.layers[0].set_size((cols_i32, rows_i32));

            // Set ice colors mode
            buffer.ice_mode = if doc.ice_colors { IceMode::Ice } else { IceMode::Blink };

            // Set 9px font (letter spacing)
            buffer.set_use_letter_spacing(doc.use_9px);

            // TODO: Set font by name when font lookup is available
            // For now, Moebius uses CP437 fonts so this is usually fine

            if buffer.layers.is_empty() {
                return;
            }

            // Resize and preallocate layer 0 for fast bulk writes.
            buffer.layers[0].preallocate_lines(cols_i32, rows_i32);

            for col in 0..(doc.columns as usize) {
                for row in 0..(doc.rows as usize) {
                    let block = doc.document.get(col).and_then(|c| c.get(row)).cloned().unwrap_or_default();

                    let mut ch = AttributedChar::default();
                    ch.ch = char::from_u32(block.code).unwrap_or(' ');
                    ch.attribute.set_foreground(block.fg as u32);
                    ch.attribute.set_background(block.bg as u32);

                    buffer.layers[0].set_char_unchecked(Position::new(col as i32, row as i32), ch);
                }
            }

            buffer.mark_dirty();

            let after_w = buffer.width();
            let after_h = buffer.height();
            let after_lines = buffer.layers.first().map(|l| l.lines.len()).unwrap_or(0);
            log::info!(
                "[COLLAB] apply_remote_document: buffer AFTER fill: width={}, height={}, layer0.lines.len()={}",
                after_w,
                after_h,
                after_lines
            );

            // Set SAUCE metadata (stored on EditState, not buffer)
            let mut sauce = icy_engine_edit::SauceMetaData::default();
            sauce.title = doc.title.clone().into();
            sauce.author = doc.author.clone().into();
            sauce.group = doc.group.clone().into();
            sauce.comments = doc.comments.lines().map(|line| line.to_string().into()).collect();
            state.set_sauce_meta(sauce);
        });

        // Update viewport size after document size changed
        self.core.update_viewport_size();
    }

    /// Apply a canvas size change (Moebius `SET_CANVAS_SIZE`).
    ///
    /// Preserves existing characters in the overlapping region and crops/extends
    /// as required.
    pub fn apply_remote_canvas_resize(&mut self, columns: u32, rows: u32) {
        use icy_engine::{AttributedChar, Line, Size};

        let new_w = columns as i32;
        let new_h = rows as i32;

        self.core.with_edit_state(|state| {
            let buffer = state.get_buffer_mut();
            let old_w = buffer.width();
            let old_h = buffer.height();

            buffer.set_size(Size::new(new_w, new_h));

            for layer in buffer.layers.iter_mut() {
                layer.set_size(Size::new(new_w, new_h));

                // Ensure we have a full line vector to work with.
                if layer.lines.len() < old_h.max(0) as usize {
                    layer.lines.resize(old_h.max(0) as usize, Line::create(old_w));
                }

                // Resize height (crop/extend)
                layer.lines.truncate(new_h.max(0) as usize);
                while layer.lines.len() < new_h.max(0) as usize {
                    layer.lines.push(Line::create(new_w));
                }

                // Resize width for each line (preserve prefix)
                for line in layer.lines.iter_mut() {
                    if line.chars.len() > new_w.max(0) as usize {
                        line.chars.truncate(new_w.max(0) as usize);
                    } else if line.chars.len() < new_w.max(0) as usize {
                        line.chars.resize(new_w.max(0) as usize, AttributedChar::invisible());
                    }
                }
            }

            buffer.mark_dirty();
        });

        // Update viewport size after document size changed
        self.core.update_viewport_size();
    }

    /// Apply SAUCE metadata change from remote user.
    /// Note: This uses set_sauce_meta directly without undo, as remote changes should not be undoable.
    pub fn apply_remote_sauce(&mut self, title: String, author: String, group: String, comments: String) {
        self.core.with_edit_state(|state| {
            let mut sauce = icy_engine_edit::SauceMetaData::default();
            sauce.title = title.into();
            sauce.author = author.into();
            sauce.group = group.into();
            sauce.comments = comments.lines().map(|line| line.to_string().into()).collect();
            state.set_sauce_meta(sauce);
        });
    }

    /// Update remote cursors from collaboration state
    pub fn set_remote_cursors(&mut self, cursors: Vec<super::widget::remote_cursors::RemoteCursor>) {
        self.core.set_remote_cursors(cursors);
    }

    /// Scroll the canvas to show the given character position (used for goto user)
    pub fn scroll_to_position(&mut self, col: i32, row: i32) {
        self.core.scroll_to_position(col, row);
    }

    pub fn zoom_info_string(&self) -> String {
        self.core.canvas.monitor_settings.read().scaling_mode.format_zoom_string()
    }

    pub fn sync_ui(&mut self) {
        self.core.sync_ui();
        let (palette, format_mode, caret_fg, caret_bg, tag_count) = self.core.with_edit_state(|state| {
            let palette = state.get_buffer().palette.clone();
            let format_mode = state.get_format_mode();
            let caret = state.get_caret();
            let tag_count = state.get_buffer().tags.len();
            (palette, format_mode, caret.attribute.foreground(), caret.attribute.background(), tag_count)
        });
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        self.palette_grid.sync_palette(&palette, palette_limit);
        self.palette_grid.set_foreground(caret_fg);
        self.palette_grid.set_background(caret_bg);

        // Clear invalid tag selections (tags may have been removed by undo).
        if let Some(tag_tool) = self.core.active_tag_tool_mut() {
            tag_tool.state_mut().selection.retain(|&idx| idx < tag_count);
        } else {
            let _ = self
                .tool_panel
                .registry
                .with_mut::<tools::TagTool, _>(|t| t.state_mut().selection.retain(|&idx| idx < tag_count));
        }

        // Tag overlays are only visible when Tag tool is active.
        if self.core.active_tag_tool().is_some() {
            self.core.update_tag_overlays();
        }
    }

    pub fn refresh_selection_display(&mut self) {
        self.core.refresh_selection_display();
    }

    pub fn status_info(&self) -> AnsiStatusInfo {
        self.core.status_info()
    }

    pub fn handle_event(&mut self, event: &iced::Event) -> bool {
        self.core.handle_event(event)
    }

    pub fn screen(&self) -> &Arc<parking_lot::Mutex<Box<dyn icy_engine::Screen>>> {
        &self.core.screen
    }

    pub fn cut(&mut self) -> Result<(), String> {
        let result = self.core.cut();
        self.refresh_selection_display();
        result
    }

    pub fn copy(&mut self) -> Result<(), String> {
        let result = self.core.copy();
        self.refresh_selection_display();
        result
    }

    pub fn paste(&mut self) -> Result<(), String> {
        self.core.paste()
    }

    pub fn mark_modified(&mut self) {
        self.core.is_modified = true;
    }

    pub fn font_tool_library(&self) -> SharedFontLibrary {
        if let Some(font) = self.core.active_font_tool() {
            return font.font_tool.font_library();
        }

        self.tool_panel
            .registry
            .get_ref::<tools::FontTool>()
            .map(|t| t.font_tool.font_library())
            .expect("FontTool should exist")
    }

    pub fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&mut self, f: F) -> T {
        self.core.with_edit_state(f)
    }

    pub fn with_edit_state_readonly<T, F: FnOnce(&EditState) -> T>(&self, f: F) -> T {
        self.core.with_edit_state_readonly(f)
    }

    /// Run a Lua script on the current buffer (for MCP/API usage)
    /// Returns the script output or an error message
    pub fn run_lua_script(&self, script: &str, undo_description: Option<&str>) -> Result<String, String> {
        let undo_desc = undo_description.unwrap_or("MCP Script");
        Plugin::run_script_string(self.screen(), script, undo_desc)
    }

    /// Get full layer data including all character cells (for MCP)
    pub fn get_layer_data(&self, layer_index: usize) -> Result<crate::mcp::types::LayerData, String> {
        use crate::mcp::types::{CharInfo, ColorInfo, LayerData};
        use icy_engine::{AttributeColor, TextPane};

        self.with_edit_state_readonly(|state| {
            let buffer = state.get_buffer();

            let layer = buffer
                .layers
                .get(layer_index)
                .ok_or_else(|| format!("Layer index {} out of range (0-{})", layer_index, buffer.layers.len().saturating_sub(1)))?;

            let size = layer.size();
            let width = size.width;
            let height = size.height;

            // Helper to convert color
            fn color_to_info(color: &AttributeColor) -> ColorInfo {
                match color {
                    AttributeColor::Rgb(r, g, b) => ColorInfo::Rgb { r: *r, g: *g, b: *b },
                    AttributeColor::Palette(idx) => ColorInfo::Palette(*idx),
                    AttributeColor::ExtendedPalette(idx) => ColorInfo::ExtendedPalette(*idx),
                    AttributeColor::Transparent => ColorInfo::Transparent,
                }
            }

            let mut chars = Vec::with_capacity((width * height) as usize);

            for y in 0..height {
                for x in 0..width {
                    let ch = layer.char_at((x, y).into());
                    let attr = ch.attribute;
                    let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);

                    chars.push(CharInfo {
                        ch: unicode_ch.to_string(),
                        fg: color_to_info(&attr.foreground_color()),
                        bg: color_to_info(&attr.background_color()),
                        font_page: attr.font_page(),
                        bold: attr.is_bold(),
                        blink: attr.is_blinking(),
                        is_visible: ch.is_visible(),
                    });
                }
            }

            Ok(LayerData {
                index: layer_index,
                title: layer.properties.title.clone(),
                is_visible: layer.properties.is_visible,
                is_locked: layer.properties.is_locked,
                is_position_locked: layer.properties.is_position_locked,
                offset_x: layer.offset().x,
                offset_y: layer.offset().y,
                width,
                height,
                transparency: layer.transparency,
                mode: format!("{:?}", layer.properties.mode),
                role: format!("{:?}", layer.role),
                chars,
            })
        })
    }

    /// Set a character at a specific position in a layer (for MCP)
    /// This operation is atomic and supports undo.
    pub fn set_char_at(&mut self, layer_index: usize, x: i32, y: i32, ch: &str, attribute: &crate::mcp::types::TextAttributeInfo) -> Result<(), String> {
        use icy_engine::{AttributeColor, AttributedChar, Position, TextAttribute, TextPane};

        // Convert attribute info to AttributeColor
        fn info_to_color(info: &crate::mcp::types::ColorInfo) -> AttributeColor {
            match info {
                crate::mcp::types::ColorInfo::Palette(idx) => AttributeColor::Palette(*idx),
                crate::mcp::types::ColorInfo::ExtendedPalette(idx) => AttributeColor::ExtendedPalette(*idx),
                crate::mcp::types::ColorInfo::Rgb { r, g, b } => AttributeColor::Rgb(*r, *g, *b),
                crate::mcp::types::ColorInfo::Transparent => AttributeColor::Transparent,
            }
        }

        let fg = info_to_color(&attribute.foreground);
        let bg = info_to_color(&attribute.background);

        let mut attr = TextAttribute::from_colors(fg, bg);
        attr.set_is_bold(attribute.bold);
        attr.set_is_blinking(attribute.blink);

        let char_value = ch.chars().next().ok_or_else(|| "Empty character string".to_string())?;

        self.with_edit_state(|state| {
            let buffer = state.get_buffer();

            if layer_index >= buffer.layers.len() {
                return Err(format!(
                    "Layer index {} out of range (0-{})",
                    layer_index,
                    buffer.layers.len().saturating_sub(1)
                ));
            }

            let converted_char = buffer.buffer_type.convert_from_unicode(char_value);
            let size = buffer.layers[layer_index].size();

            if x < 0 || x >= size.width || y < 0 || y >= size.height {
                return Err(format!("Position ({}, {}) out of bounds for layer size {}x{}", x, y, size.width, size.height));
            }

            let attributed_char = AttributedChar::new(converted_char, attr);
            let pos = Position::new(x, y);

            // Use atomic undo for single character set
            let _undo = state.begin_atomic_undo("MCP Set Char");
            if let Err(e) = state.set_char_at_layer_in_atomic(layer_index, pos, attributed_char) {
                log::error!("MCP set_char_at failed: {}", e);
                return Err(e.to_string());
            }
            Ok(())
        })?;

        self.sync_ui();
        Ok(())
    }

    /// Set a palette color (for MCP)
    pub fn set_palette_color(&mut self, index: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        use icy_engine::Color;

        self.core.with_edit_state_mut_shared(|state| {
            let buffer = state.get_buffer_mut();
            let palette_len = buffer.palette.len();

            if (index as usize) >= palette_len {
                return Err(format!("Palette index {} out of range (0-{})", index, palette_len.saturating_sub(1)));
            }

            buffer.palette.set_color(index as u32, Color::new(r, g, b));
            Ok(())
        })
    }

    // ═══════════════════════════════════════════════════════════════════
    // MCP helpers (ANSI editor)
    // ═══════════════════════════════════════════════════════════════════

    fn mcp_color_to_info(color: &icy_engine::AttributeColor) -> crate::mcp::types::ColorInfo {
        use crate::mcp::types::ColorInfo;
        match color {
            icy_engine::AttributeColor::Rgb(r, g, b) => ColorInfo::Rgb { r: *r, g: *g, b: *b },
            icy_engine::AttributeColor::Palette(idx) => ColorInfo::Palette(*idx),
            icy_engine::AttributeColor::ExtendedPalette(idx) => ColorInfo::ExtendedPalette(*idx),
            icy_engine::AttributeColor::Transparent => ColorInfo::Transparent,
        }
    }

    fn mcp_info_to_color(info: &crate::mcp::types::ColorInfo) -> icy_engine::AttributeColor {
        match info {
            crate::mcp::types::ColorInfo::Palette(idx) => icy_engine::AttributeColor::Palette(*idx),
            crate::mcp::types::ColorInfo::ExtendedPalette(idx) => icy_engine::AttributeColor::ExtendedPalette(*idx),
            crate::mcp::types::ColorInfo::Rgb { r, g, b } => icy_engine::AttributeColor::Rgb(*r, *g, *b),
            crate::mcp::types::ColorInfo::Transparent => icy_engine::AttributeColor::Transparent,
        }
    }

    fn mcp_attr_to_info(attr: &icy_engine::TextAttribute) -> crate::mcp::types::TextAttributeInfo {
        crate::mcp::types::TextAttributeInfo {
            foreground: Self::mcp_color_to_info(&attr.foreground_color()),
            background: Self::mcp_color_to_info(&attr.background_color()),
            bold: attr.is_bold(),
            blink: attr.is_blinking(),
        }
    }

    fn mcp_text_attr_from_info(info: &crate::mcp::types::TextAttributeInfo) -> icy_engine::TextAttribute {
        let fg = Self::mcp_info_to_color(&info.foreground);
        let bg = Self::mcp_info_to_color(&info.background);
        let mut attr = icy_engine::TextAttribute::from_colors(fg, bg);
        attr.set_is_bold(info.bold);
        attr.set_is_blinking(info.blink);
        attr
    }

    /// Get the current screen as ANSI or ASCII
    pub fn get_screen(&self, format: &crate::mcp::types::AnsiScreenFormat) -> Result<String, String> {
        use icy_engine::TextPane;
        use icy_engine::formats::FileFormat;

        self.with_edit_state_readonly(|state| {
            let buffer = state.get_buffer();
            match format {
                crate::mcp::types::AnsiScreenFormat::Ascii => {
                    // Compose from the merged buffer and convert CP437 bytes to Unicode.
                    let mut out = String::new();
                    for y in 0..buffer.height() {
                        for x in 0..buffer.width() {
                            let ch = buffer.char_at((x, y).into());
                            out.push(buffer.buffer_type.convert_to_unicode(ch.ch));
                        }
                        if y + 1 < buffer.height() {
                            out.push('\n');
                        }
                    }
                    Ok(out)
                }
                crate::mcp::types::AnsiScreenFormat::Ansi => {
                    let options = icy_engine::AnsiSaveOptionsV2::default();
                    let bytes = FileFormat::Ansi.to_bytes(buffer, &options).map_err(|e| e.to_string())?;

                    // Convert CP437 bytes to Unicode while preserving control codes and ESC sequences.
                    let mut out = String::with_capacity(bytes.len());
                    for b in bytes {
                        if b < 0x80 {
                            out.push(b as char);
                        } else {
                            out.push(buffer.buffer_type.convert_to_unicode(b as char));
                        }
                    }
                    Ok(out)
                }
            }
        })
    }

    /// Get caret position and attribute (for MCP)
    pub fn get_caret_info(&self) -> Result<crate::mcp::types::CaretInfo, String> {
        self.with_edit_state_readonly(|state| {
            let caret = state.get_caret();
            let current_layer = state.get_current_layer().map_err(|e| e.to_string())?;
            let layer_offset = state.get_buffer().layers.get(current_layer).map(|l| l.offset()).unwrap_or_default();

            Ok(crate::mcp::types::CaretInfo {
                x: caret.x,
                y: caret.y,
                doc_x: caret.x + layer_offset.x,
                doc_y: caret.y + layer_offset.y,
                attribute: Self::mcp_attr_to_info(&caret.attribute),
                insert_mode: caret.insert_mode,
                font_page: caret.font_page(),
            })
        })
    }

    /// Set caret position and attribute (for MCP)
    pub fn set_caret(&mut self, x: i32, y: i32, attribute: &crate::mcp::types::TextAttributeInfo) -> Result<(), String> {
        use icy_engine::Position;

        let attr = Self::mcp_text_attr_from_info(attribute);

        self.with_edit_state(|state| {
            state.set_caret_position(Position::new(x, y));
            state.set_caret_attribute(attr);
        });
        self.sync_ui();
        Ok(())
    }

    /// List layer metadata (for MCP)
    pub fn list_layers(&self) -> Result<Vec<crate::mcp::types::LayerInfo>, String> {
        self.with_edit_state_readonly(|state| {
            let buffer = state.get_buffer();
            Ok(buffer
                .layers
                .iter()
                .enumerate()
                .map(|(index, layer)| {
                    let size = layer.size();
                    crate::mcp::types::LayerInfo {
                        index,
                        title: layer.properties.title.clone(),
                        is_visible: layer.properties.is_visible,
                        is_locked: layer.properties.is_locked,
                        is_position_locked: layer.properties.is_position_locked,
                        offset_x: layer.offset().x,
                        offset_y: layer.offset().y,
                        width: size.width,
                        height: size.height,
                        transparency: layer.transparency,
                        mode: format!("{:?}", layer.properties.mode),
                        role: format!("{:?}", layer.role),
                    }
                })
                .collect())
        })
    }

    /// Add a new layer after the given layer index (for MCP)
    pub fn add_layer(&mut self, after_layer: usize) -> Result<usize, String> {
        let mut new_index: Option<usize> = None;
        self.with_edit_state(|state| {
            if let Err(e) = state.add_new_layer(after_layer) {
                log::error!("MCP add_layer failed: {}", e);
            } else {
                new_index = state.get_current_layer().ok();
            }
        });
        self.sync_ui();
        new_index.ok_or_else(|| "Failed to determine new layer index".to_string())
    }

    pub fn delete_layer(&mut self, layer: usize) -> Result<(), String> {
        self.with_edit_state(|state| {
            if let Err(e) = state.remove_layer(layer) {
                log::error!("MCP delete_layer failed: {}", e);
            }
        });
        self.sync_ui();
        Ok(())
    }

    pub fn set_layer_props(&mut self, req: &crate::mcp::types::AnsiSetLayerPropsRequest) -> Result<(), String> {
        use icy_engine::Position;

        // Update Properties via undo-enabled op, transparency directly.
        self.with_edit_state(|state| {
            let buffer = state.get_buffer();
            let Some(layer) = buffer.layers.get(req.layer) else {
                log::error!("MCP set_layer_props: invalid layer {}", req.layer);
                return;
            };

            let mut props = layer.properties.clone();

            if let Some(title) = &req.title {
                props.title = title.clone();
            }
            if let Some(is_visible) = req.is_visible {
                props.is_visible = is_visible;
            }
            if let Some(is_locked) = req.is_locked {
                props.is_locked = is_locked;
            }
            if let Some(is_position_locked) = req.is_position_locked {
                props.is_position_locked = is_position_locked;
            }
            if req.offset_x.is_some() || req.offset_y.is_some() {
                let x = req.offset_x.unwrap_or(props.offset.x);
                let y = req.offset_y.unwrap_or(props.offset.y);
                props.offset = Position::new(x, y);
            }

            if let Err(e) = state.update_layer_properties(req.layer, props) {
                log::error!("MCP update_layer_properties failed: {}", e);
            }

            if let Some(transparency) = req.transparency {
                // Not currently undo-tracked, but requested by MCP API.
                if let Some(layer_mut) = state.get_buffer_mut().layers.get_mut(req.layer) {
                    layer_mut.transparency = transparency;
                }
            }
        });
        self.sync_ui();
        Ok(())
    }

    pub fn merge_down_layer(&mut self, layer: usize) -> Result<(), String> {
        self.with_edit_state(|state| {
            if let Err(e) = state.merge_layer_down(layer) {
                log::error!("MCP merge_down_layer failed: {}", e);
            }
        });
        self.sync_ui();
        Ok(())
    }

    pub fn move_layer(&mut self, layer: usize, direction: crate::mcp::types::LayerMoveDirection) -> Result<(), String> {
        self.with_edit_state(|state| {
            let res = match direction {
                crate::mcp::types::LayerMoveDirection::Up => state.raise_layer(layer),
                crate::mcp::types::LayerMoveDirection::Down => state.lower_layer(layer),
            };
            if let Err(e) = res {
                log::error!("MCP move_layer failed: {}", e);
            }
        });
        self.sync_ui();
        Ok(())
    }

    pub fn resize_buffer(&mut self, width: i32, height: i32) -> Result<(), String> {
        self.with_edit_state(|state| {
            if let Err(e) = state.resize_buffer(false, (width, height)) {
                log::error!("MCP resize_buffer failed: {}", e);
            }
        });
        self.sync_ui();
        Ok(())
    }

    pub fn get_region(&self, layer: usize, x: i32, y: i32, width: i32, height: i32) -> Result<crate::mcp::types::RegionData, String> {
        use icy_engine::TextPane;

        self.with_edit_state_readonly(|state| {
            let buffer = state.get_buffer();
            let layer_ref = buffer.layers.get(layer).ok_or_else(|| format!("Layer index {} out of range", layer))?;

            let size = layer_ref.size();
            if x < 0 || y < 0 || width < 0 || height < 0 || x + width > size.width || y + height > size.height {
                return Err(format!(
                    "Region ({},{}) {}x{} out of bounds for layer size {}x{}",
                    x, y, width, height, size.width, size.height
                ));
            }

            let mut chars = Vec::with_capacity((width * height) as usize);
            for yy in 0..height {
                for xx in 0..width {
                    let ch = layer_ref.char_at((x + xx, y + yy).into());
                    let attr = ch.attribute;
                    let unicode_ch = buffer.buffer_type.convert_to_unicode(ch.ch);
                    chars.push(crate::mcp::types::CharInfo {
                        ch: unicode_ch.to_string(),
                        fg: Self::mcp_color_to_info(&attr.foreground_color()),
                        bg: Self::mcp_color_to_info(&attr.background_color()),
                        font_page: attr.font_page(),
                        bold: attr.is_bold(),
                        blink: attr.is_blinking(),
                        is_visible: ch.is_visible(),
                    });
                }
            }

            Ok(crate::mcp::types::RegionData {
                layer,
                x,
                y,
                width,
                height,
                chars,
            })
        })
    }

    pub fn set_region(&mut self, layer: usize, x: i32, y: i32, width: i32, height: i32, chars: &[crate::mcp::types::CharInfo]) -> Result<(), String> {
        use icy_engine::{AttributedChar, Position, TextAttribute, TextPane};

        if (width * height) as usize != chars.len() {
            return Err(format!("chars length mismatch: expected {}, got {}", (width * height), chars.len()));
        }

        self.with_edit_state(|state| {
            // Validate layer index
            let buffer = state.get_buffer();
            if layer >= buffer.layers.len() {
                return Err(format!("Layer index {} out of range", layer));
            }

            let size = buffer.layers[layer].size();
            if x < 0 || y < 0 || width < 0 || height < 0 || x + width > size.width || y + height > size.height {
                return Err(format!(
                    "Region ({},{}) {}x{} out of bounds for layer size {}x{}",
                    x, y, width, height, size.width, size.height
                ));
            }

            let buffer_type = buffer.buffer_type;

            // Begin atomic undo group for all character changes
            let _undo = state.begin_atomic_undo("MCP Set Region");

            let mut i = 0usize;
            for yy in 0..height {
                for xx in 0..width {
                    let cell = &chars[i];
                    i += 1;

                    let pos = Position::new(x + xx, y + yy);

                    let new = if !cell.is_visible {
                        AttributedChar::invisible()
                    } else {
                        let ch_value = cell.ch.chars().next().ok_or_else(|| "Empty character string".to_string())?;
                        let converted_char = buffer_type.convert_from_unicode(ch_value);
                        let fg = Self::mcp_info_to_color(&cell.fg);
                        let bg = Self::mcp_info_to_color(&cell.bg);
                        let mut attr = TextAttribute::from_colors(fg, bg);
                        attr.set_is_bold(cell.bold);
                        attr.set_is_blinking(cell.blink);
                        attr.set_font_page(cell.font_page);
                        AttributedChar::new(converted_char, attr)
                    };

                    // Push undo operation for this character using the new layer-specific method
                    if let Err(e) = state.set_char_at_layer_in_atomic(layer, pos, new) {
                        log::error!("MCP set_region set_char_at_layer_in_atomic failed: {}", e);
                    }
                }
            }
            Ok(())
        })?;

        self.sync_ui();
        Ok(())
    }

    pub fn get_selection(&self) -> Result<Option<crate::mcp::types::SelectionInfo>, String> {
        self.with_edit_state_readonly(|state| {
            let Some(sel) = state.selection() else { return Ok(None) };

            let rect = sel.as_rectangle();
            Ok(Some(crate::mcp::types::SelectionInfo {
                anchor_x: sel.anchor.x,
                anchor_y: sel.anchor.y,
                lead_x: sel.lead.x,
                lead_y: sel.lead.y,
                shape: format!("{:?}", sel.shape),
                locked: sel.locked,
                bounds: crate::mcp::types::RectangleInfo {
                    x: rect.left(),
                    y: rect.top(),
                    width: rect.width(),
                    height: rect.height(),
                },
            }))
        })
    }

    pub fn set_selection(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
        use icy_engine::Rectangle;
        self.with_edit_state(|state| {
            let _ = state.set_selection(Rectangle::from(x, y, width, height));
        });
        self.refresh_selection_display();
        self.sync_ui();
        Ok(())
    }

    pub fn clear_selection(&mut self) -> Result<(), String> {
        self.with_edit_state(|state| {
            let _ = state.clear_selection();
        });
        self.refresh_selection_display();
        self.sync_ui();
        Ok(())
    }

    pub fn selection_action(&mut self, action: &str) -> Result<(), String> {
        let action = action.trim().to_lowercase();
        self.with_edit_state(|state| {
            let res = match action.as_str() {
                "flip_x" => state.flip_x(),
                "flip_y" => state.flip_y(),
                "crop" => state.crop(),
                "justify_left" => state.justify_left(),
                "justify_center" | "center" => state.center(),
                "justify_right" => state.justify_right(),
                "justify_line_left" => state.justify_line_left(),
                "justify_line_center" | "center_line" => state.center_line(),
                "justify_line_right" => state.justify_line_right(),
                "delete_selection" | "delete" => state.erase_selection(),
                "deselect" | "clear" => state.clear_selection(),
                _ => Ok(()),
            };
            if let Err(e) = res {
                log::error!("MCP selection_action '{}' failed: {}", action, e);
            }
        });
        self.refresh_selection_display();
        self.sync_ui();
        Ok(())
    }

    pub fn update(&mut self, message: AnsiEditorMessage, dialogs: &mut DialogStack<Message>, plugins: &Arc<Vec<Plugin>>) -> Task<AnsiEditorMessage> {
        match message {
            // ═══════════════════════════════════════════════════════════════════
            // Dialog-related messages (formerly intercepted by MainWindow)
            // ═══════════════════════════════════════════════════════════════════
            AnsiEditorMessage::EditLayer(layer_index) => {
                // Forward to ShowEditLayerDialog
                return self.update(AnsiEditorMessage::ShowEditLayerDialog(layer_index), dialogs, plugins);
            }
            AnsiEditorMessage::ShowEditLayerDialog(layer_index) => {
                let data = self.with_edit_state_readonly(|state| {
                    let buffer = state.get_buffer();
                    buffer.layers.get(layer_index).map(|layer| (layer.properties.clone(), layer.size()))
                });
                if let Some((properties, size)) = data {
                    dialogs.push(EditLayerDialog::new(layer_index, properties, size));
                }
                Task::none()
            }
            AnsiEditorMessage::EditLayerDialog(_) => {
                // Handled by DialogStack
                Task::none()
            }
            AnsiEditorMessage::ApplyEditLayer(ref result) => {
                self.with_edit_state(|state| {
                    if let Err(e) = state.update_layer_properties(result.layer_index, result.properties.clone()) {
                        log::error!("Failed to update layer properties: {}", e);
                    }
                    if let Some(new_size) = result.new_size {
                        if let Err(e) = state.set_layer_size(result.layer_index, (new_size.width, new_size.height)) {
                            log::error!("Failed to resize layer: {}", e);
                        }
                    }
                });
                Task::none()
            }
            AnsiEditorMessage::Core(AnsiEditorCoreMessage::TopToolbar(TopToolbarMessage::OpenFontSelector)) => {
                // Open TDF font dialog from TopToolbar
                let dialog = TdfFontSelectorDialog::new(self.font_tool_library());
                dialogs.push(dialog);
                Task::none()
            }
            AnsiEditorMessage::ShowReferenceImageDialog => {
                dialogs.push(ReferenceImageDialog::new());
                Task::none()
            }
            AnsiEditorMessage::ReferenceImageDialog(ref dialog_msg) => {
                let _ = dialogs.update(&Message::AnsiEditor(AnsiEditorMessage::ReferenceImageDialog(dialog_msg.clone())));
                Task::none()
            }
            AnsiEditorMessage::EditPalette => {
                let pal = self.with_edit_state(|state| state.get_buffer().palette.clone());
                dialogs.push(PaletteEditorDialog::new(pal));
                Task::none()
            }
            AnsiEditorMessage::SwitchFontSlot(slot) => {
                let is_double_click = self.slot_double_click.borrow_mut().is_double_click(slot);
                let dialog = self.with_edit_state(|state| {
                    state.set_caret_font_page(slot as u8);
                    is_double_click.then(|| FontSelectorDialog::new(state))
                });
                if let Some(dialog) = dialog {
                    dialogs.push(dialog);
                }
                Task::none()
            }
            AnsiEditorMessage::OpenFontSelector => {
                let is_unrestricted = self.with_edit_state_readonly(|state| state.get_format_mode() == icy_engine_edit::FormatMode::Unrestricted);
                if is_unrestricted {
                    let dialog = self.with_edit_state_readonly(FontSlotManagerDialog::new);
                    dialogs.push(dialog);
                } else {
                    let dialog = self.with_edit_state_readonly(FontSelectorDialog::new);
                    dialogs.push(dialog);
                }
                Task::none()
            }
            AnsiEditorMessage::OpenFontSelectorForSlot(slot) => {
                let dialog = self.with_edit_state(|state| {
                    state.set_caret_font_page(slot as u8);
                    FontSelectorDialog::new(state)
                });
                dialogs.push(dialog);
                Task::none()
            }
            AnsiEditorMessage::FontSelector(ref dialog_msg) => {
                let _ = dialogs.update(&Message::AnsiEditor(AnsiEditorMessage::FontSelector(dialog_msg.clone())));
                Task::none()
            }
            AnsiEditorMessage::OpenFontSlotManager => {
                let dialog = self.with_edit_state_readonly(FontSlotManagerDialog::new);
                dialogs.push(dialog);
                Task::none()
            }
            AnsiEditorMessage::FontSlotManager(ref dialog_msg) => {
                let _ = dialogs.update(&Message::AnsiEditor(AnsiEditorMessage::FontSlotManager(dialog_msg.clone())));
                Task::none()
            }
            AnsiEditorMessage::TdfFontSelector(ref dialog_msg) => {
                let _ = dialogs.update(&Message::AnsiEditor(AnsiEditorMessage::TdfFontSelector(dialog_msg.clone())));
                Task::none()
            }
            AnsiEditorMessage::PaletteEditorDialog(_) => {
                // Handled by DialogStack
                Task::none()
            }
            AnsiEditorMessage::PaletteEditorApplied(ref pal) => {
                let res = self.with_edit_state(|state| state.switch_to_palette(pal.clone()));
                if res.is_ok() {
                    self.mark_modified();
                    self.sync_ui();
                }
                Task::none()
            }
            AnsiEditorMessage::RunPlugin(id) => {
                if let Some(plugin) = plugins.get(id) {
                    let plugin = plugin.clone();
                    if let Err(err) = plugin.run_plugin(self.screen()) {
                        use crate::fl;
                        use icy_engine_gui::ui::error_dialog;
                        dialogs.push(error_dialog(fl!("error-plugin-title"), format!("{}", err), |_| Message::CloseDialog));
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::InverseSelection => {
                let _ = self.with_edit_state(|state| state.inverse_selection());
                self.refresh_selection_display();
                Task::none()
            }
            AnsiEditorMessage::PasteAsNewImage => {
                // TODO: implement paste as new image
                Task::none()
            }
            AnsiEditorMessage::ExportFile => {
                // Get buffer type and screen
                let buffer_type = self.with_edit_state(|state| state.get_buffer().buffer_type);
                let screen = self.screen().clone();

                // Get export path from file path or default
                let export_path = self
                    .file_path()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .unwrap_or("export")
                    .to_string();

                // Get export directory from file path or current directory
                let export_dir = self
                    .file_path()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

                dialogs.push(
                    export_dialog_with_defaults_from_msg(
                        export_path,
                        buffer_type,
                        screen,
                        move || export_dir.clone(),
                        (Message::ExportDialog, |msg: &Message| match msg {
                            Message::ExportDialog(inner) => Some(inner),
                            _ => None,
                        }),
                    )
                    .on_confirm(Message::ExportComplete)
                    .on_cancel(|| Message::CloseDialog),
                );
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════════
            // Tool and panel messages
            // ═══════════════════════════════════════════════════════════════════
            AnsiEditorMessage::SwitchTool(tool) => {
                let reg = &mut self.tool_panel.registry;
                self.core.change_tool(reg, tool);
                self.tool_panel.set_tool(self.core.current_tool_for_panel());

                // Send hide cursor if switched from cursor-showing to non-cursor tool
                // (Collaboration sync now handled via undo stack)

                Task::none()
            }
            AnsiEditorMessage::ToolPanel(msg) => {
                // Keep tool panel internal animation state in sync.
                let _ = self.tool_panel.update(msg.clone());

                if let ToolPanelMessage::ClickSlot(slot) = msg {
                    // Check if old tool showed cursor
                    let old_shows_cursor = self.core.current_tool_shows_cursor();

                    let current_tool = self.core.current_tool_for_panel();
                    let new_tool = self.tool_panel.registry.click_tool_slot(slot, current_tool);
                    {
                        let reg = &mut self.tool_panel.registry;
                        self.core.change_tool(reg, tools::ToolId::Tool(new_tool));
                    }
                    // Tool changes may be blocked, so always sync from core.
                    self.tool_panel.set_tool(self.core.current_tool_for_panel());

                    // Check if new tool shows cursor
                    let new_shows_cursor = self.core.current_tool_shows_cursor();

                    // (Collaboration sync now handled via undo stack)
                    let _ = (old_shows_cursor, new_shows_cursor);
                }

                Task::none()
            }
            AnsiEditorMessage::SelectTool(slot) => {
                // Check if old tool showed cursor
                let old_shows_cursor = self.core.current_tool_shows_cursor();

                let current_tool = self.core.current_tool_for_panel();
                let new_tool = self.tool_panel.registry.click_tool_slot(slot, current_tool);
                {
                    let reg = &mut self.tool_panel.registry;
                    self.core.change_tool(reg, tools::ToolId::Tool(new_tool));
                }
                self.tool_panel.set_tool(self.core.current_tool_for_panel());

                // Check if new tool shows cursor
                let new_shows_cursor = self.core.current_tool_shows_cursor();

                // (Collaboration sync now handled via undo stack)
                let _ = (old_shows_cursor, new_shows_cursor);

                Task::none()
            }
            AnsiEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        self.core.with_edit_state(|state| state.set_caret_foreground(color));
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        self.core.with_edit_state(|state| state.set_caret_background(color));
                        self.palette_grid.set_background(color);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::PasteControls(msg) => {
                // Convert paste controls messages to core messages
                let core_msg = match msg {
                    PasteControlsMessage::Anchor => AnsiEditorCoreMessage::TopToolbar(TopToolbarMessage::PasteAnchor),
                    PasteControlsMessage::Cancel => AnsiEditorCoreMessage::TopToolbar(TopToolbarMessage::PasteCancel),
                };
                self.update(AnsiEditorMessage::Core(core_msg), dialogs, plugins)
            }
            AnsiEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        self.core.color_switcher.start_swap_animation();
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        let (fg, bg) = self.core.with_edit_state(|state| state.swap_caret_colors());
                        self.palette_grid.set_foreground(fg);
                        self.palette_grid.set_background(bg);
                        self.core.color_switcher.confirm_swap();
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        self.core.with_edit_state(|state| state.reset_caret_colors());
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                    }
                    ColorSwitcherMessage::Tick(delta) => {
                        if self.core.color_switcher.tick(delta) {
                            return Task::done(AnsiEditorMessage::ColorSwitcher(ColorSwitcherMessage::AnimationComplete));
                        }
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::RightPanel(msg) => {
                // Update UI state first.
                let task = self.right_panel.update(msg.clone()).map(AnsiEditorMessage::RightPanel);

                // Then translate panel interactions into core actions.
                match msg {
                    RightPanelMessage::Minimap(minimap_msg) => {
                        match minimap_msg {
                            MinimapMessage::ScrollTo { norm_x, norm_y } => {
                                self.core.scroll_canvas_to_normalized(norm_x, norm_y);
                            }
                            MinimapMessage::Scroll(_dy) => {
                                // handled internally by the minimap view
                            }
                            MinimapMessage::EnsureViewportVisible(..) => {
                                // handled internally by the minimap view
                            }
                        }
                    }
                    RightPanelMessage::Layers(layer_msg) => {
                        return match layer_msg {
                            LayerMessage::Select(idx) => self.core.update(AnsiEditorCoreMessage::SelectLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::ToggleVisibility(idx) => {
                                self.core.update(AnsiEditorCoreMessage::ToggleLayerVisibility(idx)).map(AnsiEditorMessage::Core)
                            }
                            LayerMessage::Add => self.core.update(AnsiEditorCoreMessage::AddLayer).map(AnsiEditorMessage::Core),
                            LayerMessage::Remove(idx) => self.core.update(AnsiEditorCoreMessage::RemoveLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MoveUp(idx) => self.core.update(AnsiEditorCoreMessage::MoveLayerUp(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MoveDown(idx) => self.core.update(AnsiEditorCoreMessage::MoveLayerDown(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::EditLayer(idx) => Task::done(AnsiEditorMessage::EditLayer(idx)),
                            LayerMessage::Duplicate(idx) => self.core.update(AnsiEditorCoreMessage::DuplicateLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MergeDown(idx) => self.core.update(AnsiEditorCoreMessage::MergeLayerDown(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::Clear(idx) => self.core.update(AnsiEditorCoreMessage::ClearLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::Rename(_idx, _name) => Task::none(),
                            // Paste mode messages - forward to TopToolbar paste actions
                            LayerMessage::PasteKeepAsLayer => self
                                .core
                                .update(AnsiEditorCoreMessage::TopToolbar(super::TopToolbarMessage::PasteKeepAsLayer))
                                .map(AnsiEditorMessage::Core),
                            LayerMessage::PasteCancel => self
                                .core
                                .update(AnsiEditorCoreMessage::TopToolbar(super::TopToolbarMessage::PasteCancel))
                                .map(AnsiEditorMessage::Core),
                        };
                    }
                    RightPanelMessage::PaneResized(_) => {}
                }

                task
            }

            // Core messages - forward to AnsiEditorCore
            AnsiEditorMessage::Core(core_msg) => {
                let task = self.core.update(core_msg);

                // Check for pending tool switches from core
                if let Some(tool_id) = self.core.take_pending_tool_switch() {
                    return Task::batch(vec![task.map(AnsiEditorMessage::Core), Task::done(AnsiEditorMessage::SwitchTool(tool_id))]);
                }

                // Keep chrome widgets in sync with core state
                self.tool_panel.set_tool(self.core.current_tool_for_panel());

                task.map(AnsiEditorMessage::Core)
            }

            // Chat panel message - bubbles up to MainWindow
            AnsiEditorMessage::ChatPanel(_) => Task::done(message),

            AnsiEditorMessage::CenterPaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.center_panes.resize(split, ratio);
                Task::none()
            }

            // No-op - do nothing
            AnsiEditorMessage::Noop => Task::none(),
        }
    }

    /// Render the editor view with Moebius-style layout:
    /// - Left sidebar: Palette (vertical) + Tool icons
    /// - Top toolbar: Color switcher + Tool-specific options
    /// - Center: Canvas (with optional chat panel below when connected)
    /// - Right panel: Minimap, Layers, Channels
    ///
    /// In collaboration mode (`collaboration` is Some), the editor can show a Moebius-style
    /// bottom chat pane with a draggable splitter.
    pub fn view<'a>(&'a self, collaboration: Option<&'a crate::ui::collaboration::state::CollaborationState>) -> Element<'a, AnsiEditorMessage> {
        let editor = &self.core;
        // === LEFT SIDEBAR ===
        // Fixed sidebar width - palette and tool panel adapt to this
        let sidebar_width = constants::LEFT_BAR_WIDTH;

        // Get caret colors from the edit state (also used for palette mode decisions)
        let (caret_fg, caret_bg, format_mode) = {
            let mut screen_guard = editor.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            // Hide caret in paste mode, otherwise show if no selection
            let caret_visible = !editor.is_paste_mode() && state.selection().is_none();
            state.set_caret_visible(caret_visible);
            let caret = state.get_caret();
            let format_mode = state.get_format_mode();
            let fg = caret.attribute.foreground();
            let bg = caret.attribute.background();
            (fg, bg, format_mode)
        };

        // Palette grid - adapts to sidebar width
        // In XBinExtended only 8 colors are available
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        let palette_view = self
            .palette_grid
            .view_with_width(sidebar_width, palette_limit)
            .map(AnsiEditorMessage::PaletteGrid);

        // Tool panel - calculate columns based on sidebar width
        // Use theme's main area background color
        let bg_weakest = main_area_background(&Theme::Dark);

        // In paste mode, show paste controls instead of tool panel
        let left_sidebar: iced::widget::Column<'_, AnsiEditorMessage> = if editor.is_paste_mode() {
            let paste_controls = self.paste_controls.view(sidebar_width, bg_weakest).map(AnsiEditorMessage::PasteControls);
            column![palette_view, paste_controls].spacing(4)
        } else {
            let tool_panel = self.tool_panel.view_with_config(sidebar_width, bg_weakest).map(AnsiEditorMessage::ToolPanel);
            column![palette_view, tool_panel].spacing(4)
        };

        // === TOP TOOLBAR (with color switcher on the left) ===

        // Color switcher (classic icy_draw style) - shows caret's foreground/background colors
        let color_switcher = editor.color_switcher.view(caret_fg, caret_bg).map(AnsiEditorMessage::ColorSwitcher);

        // Get FKeys and font/palette for toolbar
        let (fkeys, current_font, palette) = {
            let opts = editor.options.read();
            let fkeys = opts.fkeys.clone();

            let mut screen_guard = editor.screen.lock();
            let state = screen_guard
                .as_any_mut()
                .downcast_mut::<EditState>()
                .expect("AnsiEditor screen should always be EditState");
            let buffer = state.get_buffer();
            let caret = state.get_caret();
            let font_page = caret.font_page();
            let font = buffer.font(font_page).or_else(|| buffer.font(0)).cloned();
            let palette = buffer.palette.clone();
            (fkeys, font, palette)
        };

        // Tag toolbar info (selection, add-mode) lives in TagTool.
        let (tag_add_mode, tag_selection, selected_tag_info) = if let Some(tag_tool) = editor.active_tag_tool() {
            let selection = tag_tool.state().selection.clone();
            let add_mode = tag_tool.state().add_new_index.is_some();

            let selected_tag_info = if selection.len() == 1 {
                let idx = selection[0];
                let mut screen_guard = editor.screen.lock();
                if let Some(state) = screen_guard.as_any_mut().downcast_mut::<EditState>() {
                    state.get_buffer().tags.get(idx).map(|tag| widget::toolbar::top::SelectedTagInfo {
                        position: tag.position,
                        replacement: tag.replacement_value.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            };

            (add_mode, selection, selected_tag_info)
        } else {
            (false, Vec::new(), None)
        };

        let view_ctx = tools::ToolViewContext {
            theme: Theme::Dark,
            fkeys: fkeys.clone(),
            font: current_font,
            palette: palette.clone(),
            caret_fg,
            caret_bg,
            tag_add_mode,
            selected_tag: selected_tag_info,
            tag_selection_count: tag_selection.len(),
        };

        let top_toolbar_content: Element<'_, AnsiEditorMessage> = if editor.is_paste_mode() {
            editor
                .view_paste_toolbar(&view_ctx)
                .map(|m| AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToolMessage(m)))
        } else {
            editor
                .view_current_tool_toolbar(&view_ctx)
                .map(|m| AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToolMessage(m)))
        };

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        let top_toolbar = row![color_switcher, top_toolbar_content].spacing(4).align_y(Alignment::Start);

        // === CENTER: Canvas (+ optional chat) ===
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache.
        // If chat is visible, we use a PaneGrid splitter (like the right panel).

        let center_content: Element<'_, AnsiEditorMessage> = if let Some(collab) = collaboration {
            if collab.chat_visible {
                let pane_grid: Element<'_, AnsiEditorMessage> = pane_grid::PaneGrid::new(&self.center_panes, |_id, pane, _is_maximized| {
                    let content: Element<'_, AnsiEditorMessage> = match pane {
                        CenterPane::Canvas => self.core.view().map(AnsiEditorMessage::Core),
                        CenterPane::Chat => crate::ui::collaboration::view_chat_panel(collab, &collab.chat_input).map(|m| match m {
                            Message::ChatPanel(msg) => AnsiEditorMessage::ChatPanel(msg),
                            _ => AnsiEditorMessage::Noop,
                        }),
                    };
                    pane_grid::Content::new(content)
                })
                .on_resize(10, AnsiEditorMessage::CenterPaneResized)
                .spacing(crate::ui::editor::ansi::widget::right_panel::RIGHT_PANEL_PANE_SPACING)
                .into();

                container(pane_grid).width(Length::Fill).height(Length::Fill).into()
            } else {
                self.core.view().map(AnsiEditorMessage::Core)
            }
        } else {
            self.core.view().map(AnsiEditorMessage::Core)
        };

        // === RIGHT PANEL ===
        // Right panel created AFTER canvas because minimap uses Terminal's render cache
        // which is populated when canvas.view() calls the Terminal shader

        // Compute viewport info for the minimap from the canvas terminal
        let viewport_info = editor.compute_viewport_info();
        // Pass the terminal's render cache to the minimap for shared texture access
        let render_cache = &editor.canvas.terminal.render_cache;
        let paste_mode = editor.is_paste_mode();
        let network_mode = collaboration.is_some();
        let right_panel = self
            .right_panel
            .view(&editor.screen, &viewport_info, Some(render_cache), paste_mode, network_mode)
            .map(AnsiEditorMessage::RightPanel);

        // Main layout:
        // Left column: toolbar on top, then left sidebar + canvas (with optional chat below)
        // Right: right panel spanning full height

        let left_content_row = row![
            // Left sidebar - dynamic width based on palette size
            container(left_sidebar).width(Length::Fixed(sidebar_width)),
            // Center - canvas with optional chat below
            center_content,
        ];

        let left_column = column![
            // Top toolbar - full width of left area
            container(top_toolbar)
                .width(Length::Fill)
                .height(Length::Fixed(toolbar_height))
                .style(container::rounded_box),
            // Left sidebar + canvas
            left_content_row,
        ]
        .spacing(0);

        let main_layout: Element<'_, AnsiEditorMessage> = row![
            left_column,
            // Right panel - fixed width, full height
            container(right_panel).width(Length::Fixed(RIGHT_PANEL_BASE_WIDTH)),
        ]
        .into();

        // Apply tag dialog modal overlay if active
        editor.wrap_with_modals(main_layout)
    }
}
