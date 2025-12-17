use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{column, container, row},
};
use icy_engine::formats::{FileFormat, LoadData};
use icy_engine_edit::EditState;
use icy_engine_edit::tools::{Tool, click_tool_slot};
use icy_engine_gui::theme::main_area_background;
use parking_lot::RwLock;

use crate::SharedFontLibrary;
use crate::ui::Options;

use crate::ui::{LayerMessage, MinimapMessage};

use super::*;

/// Public entrypoint for the ANSI editor mode.
///
/// Owns the core editor (`AnsiEditor`) privately and provides the surrounding
/// layout/panels (tool panel, palette grid, right panel, overlays).
pub struct AnsiEditorMainArea {
    inner: AnsiEditor,
    tool_registry: Rc<RefCell<tool_registry::ToolRegistry>>,
    /// File path (if saved)
    file_path: Option<PathBuf>,
    /// Tool panel state (left sidebar icons)
    tool_panel: ToolPanel,
    /// Palette grid
    palette_grid: PaletteGrid,
    /// Right panel state (minimap, layers)
    right_panel: RightPanel,
}

impl AnsiEditorMainArea {
    pub fn new(options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        let mut buffer = icy_engine::TextBuffer::new((80, 25));
        buffer.terminal_state.is_terminal_buffer = false;
        Self::with_buffer(buffer, None, options, font_library)
    }

    pub fn with_buffer(buffer: icy_engine::TextBuffer, file_path: Option<PathBuf>, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Self {
        let tool_registry = Rc::new(RefCell::new(tool_registry::ToolRegistry::new(font_library)));

        // Default tool is Click. Take it from the registry so it becomes the active boxed tool.
        let mut current_tool = tool_registry.borrow_mut().take_for(tools::ToolId::Tool(Tool::Click));
        if let Some(click) = current_tool.as_any_mut().downcast_mut::<tools::ClickTool>() {
            click.sync_fkey_set_from_options(&options);
        }

        let (inner, palette, format_mode) = AnsiEditor::from_buffer_inner(buffer, options, current_tool);

        let mut tool_panel = ToolPanel::new();
        tool_panel.set_tool(inner.current_tool_for_panel());

        let mut palette_grid = PaletteGrid::new();
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        palette_grid.sync_palette(&palette, palette_limit);

        Self {
            inner,
            tool_registry,
            file_path,
            tool_panel,
            palette_grid,
            right_panel: RightPanel::new(),
        }
    }

    pub fn with_file(path: PathBuf, options: Arc<RwLock<Options>>, font_library: SharedFontLibrary) -> Result<Self, String> {
        let format = FileFormat::from_path(&path).unwrap_or(FileFormat::Ansi);
        let screen = format.load(&path, Some(LoadData::default())).map_err(|e| e.to_string())?;
        Ok(Self::with_buffer(screen.buffer, Some(path), options, font_library))
    }

    pub fn load_from_autosave(
        autosave_path: &Path,
        original_path: PathBuf,
        options: Arc<RwLock<Options>>,
        font_library: SharedFontLibrary,
    ) -> Result<Self, String> {
        let data = std::fs::read(autosave_path).map_err(|e| format!("Failed to load autosave: {}", e))?;
        let format = FileFormat::from_path(&original_path).unwrap_or(FileFormat::Ansi);
        let screen = format.from_bytes(&data, Some(LoadData::default())).map_err(|e| e.to_string())?;

        let mut editor = Self::with_buffer(screen.buffer, Some(original_path), options, font_library);
        editor.inner.is_modified = true;
        Ok(editor)
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    pub fn undo_stack_len(&self) -> usize {
        self.inner.undo_stack_len()
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.inner.save(path)
    }

    /// Get bytes for autosave (saves in ICY format with thumbnail skipped for performance)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        let mut screen = self.inner.screen.lock();
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

    pub fn needs_animation(&self) -> bool {
        self.inner.needs_animation() || self.tool_panel.needs_animation() || self.inner.is_minimap_drag_active()
    }

    pub fn get_marker_menu_state(&self) -> widget::toolbar::menu_bar::MarkerMenuState {
        self.inner.get_marker_menu_state()
    }

    pub fn get_mirror_mode(&self) -> bool {
        self.inner.get_mirror_mode()
    }

    pub fn toggle_mirror_mode(&mut self) {
        self.inner.toggle_mirror_mode();
    }

    pub fn zoom_in(&mut self) {
        self.inner.canvas.zoom_in();
    }

    pub fn zoom_out(&mut self) {
        self.inner.canvas.zoom_out();
    }

    pub fn zoom_reset(&mut self) {
        self.inner.canvas.zoom_reset();
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.inner.canvas.set_zoom(zoom);
    }

    pub fn zoom_info_string(&self) -> String {
        self.inner.canvas.monitor_settings.read().scaling_mode.format_zoom_string()
    }

    pub fn sync_ui(&mut self) {
        self.inner.sync_ui();
        let (palette, format_mode, caret_fg, caret_bg, tag_count) = self.inner.with_edit_state(|state| {
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
        if let Some(tag_tool) = self.inner.active_tag_tool_mut() {
            tag_tool.state_mut().selection.retain(|&idx| idx < tag_count);
        } else {
            let _ = self
                .tool_registry
                .borrow_mut()
                .with_mut::<tools::TagTool, _>(|t| t.state_mut().selection.retain(|&idx| idx < tag_count));
        }

        // Tag overlays are only visible when Tag tool is active.
        if self.inner.active_tag_tool().is_some() {
            self.inner.update_tag_overlays();
        }
    }

    pub fn refresh_selection_display(&mut self) {
        self.inner.refresh_selection_display();
    }

    pub fn status_info(&self) -> AnsiStatusInfo {
        self.inner.status_info()
    }

    pub fn handle_event(&mut self, event: &iced::Event) -> bool {
        self.inner.handle_event(event)
    }

    pub fn screen(&self) -> &Arc<parking_lot::Mutex<Box<dyn icy_engine::Screen>>> {
        &self.inner.screen
    }

    pub fn cut(&mut self) -> Result<(), String> {
        self.inner.cut()
    }

    pub fn copy(&mut self) -> Result<(), String> {
        self.inner.copy()
    }

    pub fn paste(&mut self) -> Result<(), String> {
        self.inner.paste()
    }

    pub fn set_reference_image(&mut self, path: Option<PathBuf>, alpha: f32) {
        self.inner.set_reference_image(path, alpha);
    }

    pub fn toggle_reference_image(&mut self) {
        self.inner.toggle_reference_image();
    }

    pub fn mark_modified(&mut self) {
        self.inner.is_modified = true;
    }

    pub fn font_tool_library(&self) -> SharedFontLibrary {
        if let Some(font) = self.inner.active_font_tool() {
            return font.font_tool.font_library();
        }

        self.tool_registry
            .borrow()
            .get_ref::<tools::FontTool>()
            .map(|t| t.font_tool.font_library())
            .expect("FontTool should exist")
    }

    pub fn font_tool_select_font(&mut self, font_idx: i32) {
        if let Some(font) = self.inner.active_font_tool_mut() {
            font.select_font(font_idx);
            return;
        }

        let _ = self.tool_registry.borrow_mut().with_mut::<tools::FontTool, _>(|t| t.select_font(font_idx));
    }

    fn font_tool_is_outline_selector_open(&self) -> bool {
        if let Some(font) = self.inner.active_font_tool() {
            return font.is_outline_selector_open();
        }

        self.tool_registry
            .borrow()
            .get_ref::<tools::FontTool>()
            .map(|t| t.is_outline_selector_open())
            .unwrap_or(false)
    }

    pub fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&mut self, f: F) -> T {
        self.inner.with_edit_state(f)
    }

    pub fn with_edit_state_readonly<T, F: FnOnce(&EditState) -> T>(&self, f: F) -> T {
        self.inner.with_edit_state_readonly(f)
    }

    pub fn update(&mut self, message: AnsiEditorMessage) -> Task<AnsiEditorMessage> {
        match message {
            AnsiEditorMessage::SwitchTool(tool) => {
                let mut reg = self.tool_registry.borrow_mut();
                self.inner.change_tool(&mut *reg, tool);
                self.tool_panel.set_tool(self.inner.current_tool_for_panel());
                Task::none()
            }
            AnsiEditorMessage::OutlineSelector(msg) => {
                let options = Arc::clone(&self.inner.options);
                if let Some(font) = self.inner.active_font_tool_mut() {
                    font.handle_outline_selector_message(&options, msg);
                } else {
                    let _ = self
                        .tool_registry
                        .borrow_mut()
                        .with_mut::<tools::FontTool, _>(|t| t.handle_outline_selector_message(&options, msg));
                }
                Task::none()
            }
            AnsiEditorMessage::ToolPanel(msg) => {
                // Keep tool panel internal animation state in sync.
                let _ = self.tool_panel.update(msg.clone());

                if let ToolPanelMessage::ClickSlot(slot) = msg {
                    let current_tool = self.inner.current_tool_for_panel();
                    let new_tool = click_tool_slot(slot, current_tool);
                    let mut reg = self.tool_registry.borrow_mut();
                    self.inner.change_tool(&mut *reg, tools::ToolId::Tool(new_tool));
                    // Tool changes may be blocked, so always sync from core.
                    self.tool_panel.set_tool(self.inner.current_tool_for_panel());
                }

                Task::none()
            }
            AnsiEditorMessage::SelectTool(slot) => {
                let current_tool = self.inner.current_tool_for_panel();
                let new_tool = click_tool_slot(slot, current_tool);
                let mut reg = self.tool_registry.borrow_mut();
                self.inner.change_tool(&mut *reg, tools::ToolId::Tool(new_tool));
                self.tool_panel.set_tool(self.inner.current_tool_for_panel());
                Task::none()
            }
            AnsiEditorMessage::PaletteGrid(msg) => {
                match msg {
                    PaletteGridMessage::SetForeground(color) => {
                        self.inner.with_edit_state(|state| state.set_caret_foreground(color));
                        self.palette_grid.set_foreground(color);
                    }
                    PaletteGridMessage::SetBackground(color) => {
                        self.inner.with_edit_state(|state| state.set_caret_background(color));
                        self.palette_grid.set_background(color);
                    }
                }
                Task::none()
            }
            AnsiEditorMessage::ColorSwitcher(msg) => {
                match msg {
                    ColorSwitcherMessage::SwapColors => {
                        self.inner.color_switcher.start_swap_animation();
                    }
                    ColorSwitcherMessage::AnimationComplete => {
                        let (fg, bg) = self.inner.with_edit_state(|state| state.swap_caret_colors());
                        self.palette_grid.set_foreground(fg);
                        self.palette_grid.set_background(bg);
                        self.inner.color_switcher.confirm_swap();
                    }
                    ColorSwitcherMessage::ResetToDefault => {
                        self.inner.with_edit_state(|state| state.reset_caret_colors());
                        self.palette_grid.set_foreground(7);
                        self.palette_grid.set_background(0);
                    }
                    ColorSwitcherMessage::Tick(delta) => {
                        if self.inner.color_switcher.tick(delta) {
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
                            MinimapMessage::Click { norm_x, norm_y, .. } => {
                                self.inner.scroll_canvas_to_normalized(norm_x, norm_y);
                            }
                            MinimapMessage::Drag {
                                norm_x,
                                norm_y,
                                pointer_x,
                                pointer_y,
                            } => {
                                self.inner.set_minimap_drag_pointer(Some((pointer_x, pointer_y)));
                                self.inner.scroll_canvas_to_normalized(norm_x, norm_y);
                            }
                            MinimapMessage::DragEnd => {
                                self.inner.set_minimap_drag_pointer(None);
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
                            LayerMessage::Select(idx) => self.inner.update(AnsiEditorMessage::SelectLayer(idx)),
                            LayerMessage::ToggleVisibility(idx) => self.inner.update(AnsiEditorMessage::ToggleLayerVisibility(idx)),
                            LayerMessage::Add => self.inner.update(AnsiEditorMessage::AddLayer),
                            LayerMessage::Remove(idx) => self.inner.update(AnsiEditorMessage::RemoveLayer(idx)),
                            LayerMessage::MoveUp(idx) => self.inner.update(AnsiEditorMessage::MoveLayerUp(idx)),
                            LayerMessage::MoveDown(idx) => self.inner.update(AnsiEditorMessage::MoveLayerDown(idx)),
                            LayerMessage::EditLayer(idx) => Task::done(AnsiEditorMessage::EditLayer(idx)),
                            LayerMessage::Duplicate(idx) => self.inner.update(AnsiEditorMessage::DuplicateLayer(idx)),
                            LayerMessage::MergeDown(idx) => self.inner.update(AnsiEditorMessage::MergeLayerDown(idx)),
                            LayerMessage::Clear(idx) => self.inner.update(AnsiEditorMessage::ClearLayer(idx)),
                            LayerMessage::Rename(_idx, _name) => Task::none(),
                        };
                    }
                    RightPanelMessage::PaneResized(_) => {}
                }

                task
            }
            AnsiEditorMessage::MinimapAutoscrollTick(_delta) => {
                let Some((pointer_x, pointer_y)) = self.inner.minimap_drag_pointer() else {
                    return Task::none();
                };

                let render_cache = &self.inner.canvas.terminal.render_cache;
                if let Some((norm_x, norm_y)) =
                    self.right_panel
                        .minimap
                        .handle_click(iced::Size::new(0.0, 0.0), iced::Point::new(pointer_x, pointer_y), Some(render_cache))
                {
                    self.inner.scroll_canvas_to_normalized(norm_x, norm_y);
                }

                Task::none()
            }

            // Everything else is core-owned.
            other => {
                let task = self.inner.update(other);

                // Keep chrome widgets in sync with core state (tool changes can originate
                // from keyboard shortcuts and tool results).
                self.tool_panel.set_tool(self.inner.current_tool_for_panel());

                task
            }
        }
    }

    /// Render the editor view with Moebius-style layout:
    /// - Left sidebar: Palette (vertical) + Tool icons
    /// - Top toolbar: Color switcher + Tool-specific options
    /// - Center: Canvas
    /// - Right panel: Minimap, Layers, Channels
    pub fn view<'a>(&'a self) -> Element<'a, AnsiEditorMessage> {
        let editor = &self.inner;
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
            state.set_caret_visible(state.selection().is_none());
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
            let paste_controls = editor.view_paste_sidebar_controls();
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

        // Clone font for char selector overlay (will be used later if popup is open)
        let font_for_char_selector = current_font.clone();

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
            editor.view_paste_toolbar(&view_ctx).map(AnsiEditorMessage::ToolMessage)
        } else {
            editor.view_current_tool_toolbar(&view_ctx).map(AnsiEditorMessage::ToolMessage)
        };

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        let top_toolbar = row![color_switcher, top_toolbar_content].spacing(4).align_y(Alignment::Start);

        // === CENTER: Canvas ===
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache
        let center_area = self.inner.view();

        // === RIGHT PANEL ===
        // Right panel created AFTER canvas because minimap uses Terminal's render cache
        // which is populated when canvas.view() calls the Terminal shader

        // Compute viewport info for the minimap from the canvas terminal
        let viewport_info = editor.compute_viewport_info();
        // Pass the terminal's render cache to the minimap for shared texture access
        let render_cache = &editor.canvas.terminal.render_cache;
        let right_panel = self
            .right_panel
            .view(&editor.screen, &viewport_info, Some(render_cache))
            .map(AnsiEditorMessage::RightPanel);

        // Main layout:
        // Left column: toolbar on top, then left sidebar + canvas
        // Right: right panel spanning full height

        let left_content_row = row![
            // Left sidebar - dynamic width based on palette size
            container(left_sidebar).width(Length::Fixed(sidebar_width)),
            // Center - canvas with optional line numbers
            center_area,
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
        if let Some(tag_tool) = editor.active_tag_tool() {
            if let Some(tag_dialog) = &tag_tool.state().dialog {
                let modal_content = tag_dialog.view().map(AnsiEditorMessage::TagDialog);
                icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::TagDialog(TagDialogMessage::Cancel))
            } else if let Some(tag_list_dialog) = &tag_tool.state().list_dialog {
                let modal_content = tag_list_dialog.view().map(AnsiEditorMessage::TagListDialog);
                icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::TagListDialog(TagListDialogMessage::Close))
            } else if let Some(target) = editor.char_selector_target {
                let current_code = match target {
                    CharSelectorTarget::FKeySlot(slot) => fkeys.code_at(fkeys.current_set(), slot),
                    CharSelectorTarget::BrushChar => {
                        let ch = editor.brush_paint_char();
                        ch as u16
                    }
                };

                let selector_canvas = CharSelector::new(current_code)
                    .view(font_for_char_selector, palette.clone(), caret_fg, caret_bg)
                    .map(AnsiEditorMessage::CharSelector);

                let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, CHAR_SELECTOR_WIDTH);

                // Use modal() which closes on click outside (on_blur)
                icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::CharSelector(CharSelectorMessage::Cancel))
            } else if self.font_tool_is_outline_selector_open() {
                // Apply outline selector modal overlay if active
                let current_style = *editor.options.read().font_outline_style.read();

                let selector_canvas = OutlineSelector::new(current_style).view().map(AnsiEditorMessage::OutlineSelector);

                let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, outline_selector_width());

                // Use modal() which closes on click outside (on_blur)
                icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::OutlineSelector(OutlineSelectorMessage::Cancel))
            } else {
                main_layout
            }
        } else if let Some(target) = editor.char_selector_target {
            let current_code = match target {
                CharSelectorTarget::FKeySlot(slot) => fkeys.code_at(fkeys.current_set(), slot),
                CharSelectorTarget::BrushChar => {
                    let ch = editor.brush_paint_char();
                    ch as u16
                }
            };

            let selector_canvas = CharSelector::new(current_code)
                .view(font_for_char_selector, palette.clone(), caret_fg, caret_bg)
                .map(AnsiEditorMessage::CharSelector);

            let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, CHAR_SELECTOR_WIDTH);

            // Use modal() which closes on click outside (on_blur)
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::CharSelector(CharSelectorMessage::Cancel))
        } else if self.font_tool_is_outline_selector_open() {
            // Apply outline selector modal overlay if active
            let current_style = *editor.options.read().font_outline_style.read();

            let selector_canvas = OutlineSelector::new(current_style).view().map(AnsiEditorMessage::OutlineSelector);

            let modal_content = icy_engine_gui::ui::modal_container(selector_canvas, outline_selector_width());

            // Use modal() which closes on click outside (on_blur)
            icy_engine_gui::ui::modal(main_layout, modal_content, AnsiEditorMessage::OutlineSelector(OutlineSelectorMessage::Cancel))
        } else {
            main_layout
        }
    }
}
