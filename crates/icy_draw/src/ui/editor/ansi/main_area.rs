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
use icy_engine::TextPane;
use icy_engine_edit::EditState;
use icy_engine_edit::tools::{Tool, click_tool_slot};
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::ui::{DialogStack, export_dialog_with_defaults_from_msg};
use parking_lot::RwLock;

use crate::SharedFontLibrary;
use crate::ui::Options;
use crate::ui::editor::palette::PaletteEditorDialog;
use crate::ui::main_window::Message;
use crate::ui::widget::plugins::Plugin;
use crate::ui::{LayerMessage, MinimapMessage};

use super::*;

/// Public entrypoint for the ANSI editor mode.
///
/// Owns the core editor (`AnsiEditorCore`) privately and provides the surrounding
/// layout/panels (tool panel, palette grid, right panel, overlays).
pub struct AnsiEditorMainArea {
    core: AnsiEditorCore,
    tool_registry: Rc<RefCell<tool_registry::ToolRegistry>>,
    /// File path (if saved)
    file_path: Option<PathBuf>,
    /// Tool panel state (left sidebar icons)
    tool_panel: ToolPanel,
    /// Palette grid
    palette_grid: PaletteGrid,
    /// Right panel state (minimap, layers)
    right_panel: RightPanel,
    /// Double-click detector for font slot buttons
    slot_double_click: RefCell<icy_engine_gui::DoubleClickDetector<usize>>,
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

        let (core, palette, format_mode) = AnsiEditorCore::from_buffer_inner(buffer, options, current_tool);

        let mut tool_panel = ToolPanel::new();
        tool_panel.set_tool(core.current_tool_for_panel());

        let mut palette_grid = PaletteGrid::new();
        let palette_limit = (format_mode == icy_engine_edit::FormatMode::XBinExtended).then_some(8);
        palette_grid.sync_palette(&palette, palette_limit);

        Self {
            core,
            tool_registry,
            file_path,
            tool_panel,
            palette_grid,
            right_panel: RightPanel::new(),
            slot_double_click: RefCell::new(icy_engine_gui::DoubleClickDetector::new()),
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

    pub fn needs_animation(&self) -> bool {
        self.core.needs_animation() || self.tool_panel.needs_animation() || self.core.is_minimap_drag_active()
    }

    pub fn get_marker_menu_state(&self) -> widget::toolbar::menu_bar::MarkerMenuState {
        self.core.get_marker_menu_state()
    }

    pub fn get_mirror_mode(&self) -> bool {
        self.core.get_mirror_mode()
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
                .tool_registry
                .borrow_mut()
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
        self.core.cut()
    }

    pub fn copy(&mut self) -> Result<(), String> {
        self.core.copy()
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

        self.tool_registry
            .borrow()
            .get_ref::<tools::FontTool>()
            .map(|t| t.font_tool.font_library())
            .expect("FontTool should exist")
    }

    pub fn font_tool_select_font(&mut self, font_idx: i32) {
        if let Some(font) = self.core.active_font_tool_mut() {
            font.select_font(font_idx);
            return;
        }

        let _ = self.tool_registry.borrow_mut().with_mut::<tools::FontTool, _>(|t| t.select_font(font_idx));
    }

    pub fn with_edit_state<T, F: FnOnce(&mut EditState) -> T>(&mut self, f: F) -> T {
        self.core.with_edit_state(f)
    }

    pub fn with_edit_state_readonly<T, F: FnOnce(&EditState) -> T>(&self, f: F) -> T {
        self.core.with_edit_state_readonly(f)
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
                    state.set_caret_font_page(slot);
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
                    state.set_caret_font_page(slot);
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
                        use icy_engine_gui::ui::error_dialog;
                        use crate::fl;
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
                let mut reg = self.tool_registry.borrow_mut();
                self.core.change_tool(&mut *reg, tool);
                self.tool_panel.set_tool(self.core.current_tool_for_panel());
                Task::none()
            }
            AnsiEditorMessage::ToolPanel(msg) => {
                // Keep tool panel internal animation state in sync.
                let _ = self.tool_panel.update(msg.clone());

                if let ToolPanelMessage::ClickSlot(slot) = msg {
                    let current_tool = self.core.current_tool_for_panel();
                    let new_tool = click_tool_slot(slot, current_tool);
                    let mut reg = self.tool_registry.borrow_mut();
                    self.core.change_tool(&mut *reg, tools::ToolId::Tool(new_tool));
                    // Tool changes may be blocked, so always sync from core.
                    self.tool_panel.set_tool(self.core.current_tool_for_panel());
                }

                Task::none()
            }
            AnsiEditorMessage::SelectTool(slot) => {
                let current_tool = self.core.current_tool_for_panel();
                let new_tool = click_tool_slot(slot, current_tool);
                let mut reg = self.tool_registry.borrow_mut();
                self.core.change_tool(&mut *reg, tools::ToolId::Tool(new_tool));
                self.tool_panel.set_tool(self.core.current_tool_for_panel());
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
                            MinimapMessage::Click { norm_x, norm_y, .. } => {
                                self.core.scroll_canvas_to_normalized(norm_x, norm_y);
                            }
                            MinimapMessage::Drag {
                                norm_x,
                                norm_y,
                                pointer_x,
                                pointer_y,
                            } => {
                               // self.core.set_minimap_drag_pointer(Some((pointer_x, pointer_y)));
                                self.core.scroll_canvas_to_normalized(norm_x, norm_y);
                            }
                            MinimapMessage::DragEnd => {
                          //      self.core.set_minimap_drag_pointer(None);
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
                            LayerMessage::ToggleVisibility(idx) => self.core.update(AnsiEditorCoreMessage::ToggleLayerVisibility(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::Add => self.core.update(AnsiEditorCoreMessage::AddLayer).map(AnsiEditorMessage::Core),
                            LayerMessage::Remove(idx) => self.core.update(AnsiEditorCoreMessage::RemoveLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MoveUp(idx) => self.core.update(AnsiEditorCoreMessage::MoveLayerUp(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MoveDown(idx) => self.core.update(AnsiEditorCoreMessage::MoveLayerDown(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::EditLayer(idx) => Task::done(AnsiEditorMessage::EditLayer(idx)),
                            LayerMessage::Duplicate(idx) => self.core.update(AnsiEditorCoreMessage::DuplicateLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::MergeDown(idx) => self.core.update(AnsiEditorCoreMessage::MergeLayerDown(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::Clear(idx) => self.core.update(AnsiEditorCoreMessage::ClearLayer(idx)).map(AnsiEditorMessage::Core),
                            LayerMessage::Rename(_idx, _name) => Task::none(),
                        };
                    }
                    RightPanelMessage::PaneResized(_) => {}
                }

                task
            }
            AnsiEditorMessage::MinimapAutoscrollTick(_delta) => {
                let Some((pointer_x, pointer_y)) = self.core.minimap_drag_pointer() else {
                    return Task::none();
                };

                let render_cache = &self.core.canvas.terminal.render_cache;
                if let Some((norm_x, norm_y)) =
                    self.right_panel
                        .minimap
                        .handle_click(iced::Size::new(0.0, 0.0), iced::Point::new(pointer_x, pointer_y), Some(render_cache))
                {
                    self.core.scroll_canvas_to_normalized(norm_x, norm_y);
                }

                Task::none()
            }

            // Core messages - forward to AnsiEditorCore
            AnsiEditorMessage::Core(core_msg) => {
                let task = self.core.update(core_msg);

                // Check for pending tool switches from core
                if let Some(tool_id) = self.core.take_pending_tool_switch() {
                    return Task::batch(vec![
                        task.map(AnsiEditorMessage::Core),
                        Task::done(AnsiEditorMessage::SwitchTool(tool_id)),
                    ]);
                }

                // Keep chrome widgets in sync with core state
                self.tool_panel.set_tool(self.core.current_tool_for_panel());

                task.map(AnsiEditorMessage::Core)
            }
        }
    }

    /// Render the editor view with Moebius-style layout:
    /// - Left sidebar: Palette (vertical) + Tool icons
    /// - Top toolbar: Color switcher + Tool-specific options
    /// - Center: Canvas
    /// - Right panel: Minimap, Layers, Channels
    pub fn view<'a>(&'a self) -> Element<'a, AnsiEditorMessage> {
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
            let paste_controls = editor.view_paste_sidebar_controls().map(AnsiEditorMessage::Core);
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
            editor.view_paste_toolbar(&view_ctx).map(|m| AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToolMessage(m)))
        } else {
            editor.view_current_tool_toolbar(&view_ctx).map(|m| AnsiEditorMessage::Core(AnsiEditorCoreMessage::ToolMessage(m)))
        };

        let toolbar_height = constants::TOP_CONTROL_TOTAL_HEIGHT;

        let top_toolbar = row![color_switcher, top_toolbar_content].spacing(4).align_y(Alignment::Start);

        // === CENTER: Canvas ===
        // Canvas is created FIRST so Terminal's shader renders and populates the shared cache
        let center_area = self.core.view().map(AnsiEditorMessage::Core);

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
        editor.wrap_with_modals(main_layout)
    }
}
