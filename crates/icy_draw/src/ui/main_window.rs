use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    add_child, model::Tool, plugins::Plugin, util::autosave, AnsiEditor, AskCloseFileDialog, BitFontEditor, ChannelToolWindow, CharFontEditor, Commands,
    Document, DocumentBehavior, DocumentTab, LayerToolWindow, Message, MinimapToolWindow, ModalDialog, MostRecentlyUsedFiles, SettingsDialog, ToolBehavior,
    ToolTab, TopBar, SETTINGS,
};
use eframe::egui::{Button, PointerButton};
use eframe::{
    egui::{self, Key, Response, SidePanel, Ui},
    epaint::FontId,
};
use egui::{mutex::Mutex, Layout, Modifiers, Pos2, Rect, TextStyle, Vec2, WidgetText};
use egui_tiles::{Container, TileId};
use glow::Context;
use i18n_embed_fl::fl;
use icy_engine::{font::TheDrawFont, BitFont, Buffer, BufferType, EngineResult, Palette, TextAttribute, TextPane};

use super::KeyBindings;

pub struct MainWindow<'a> {
    pub document_tree: egui_tiles::Tree<DocumentTab>,
    pub tool_tree: egui_tiles::Tree<ToolTab>,
    pub toasts: egui_notify::Toasts,

    pub document_behavior: DocumentBehavior,
    pub tool_behavior: ToolBehavior,
    pub gl: Arc<Context>,
    title: String,

    dialog_open: bool,
    modal_dialog: Option<Box<dyn ModalDialog>>,
    id: usize,

    pub current_id: Option<TileId>,

    pub allowed_to_close: bool,
    pub request_close: bool,
    pub close_all_requested: bool,
    pub top_bar: TopBar,
    pub left_panel: bool,
    pub right_panel: bool,
    pub bottom_panel: bool,

    pub show_settings: bool,
    pub settings_dialog: SettingsDialog,
    pub commands: Vec<Box<Commands>>,
    pub last_command_update: Instant,
    pub is_fullscreen: bool,
    pub set_fullscreen_opt: Option<bool>,

    pub in_open_file_mode: bool,
    pub open_file_window: icy_view_gui::MainWindow<'a>,

    pub plugins: Vec<Plugin>,
    pub key_bindings: KeyBindings,
    pub mru_files: MostRecentlyUsedFiles,
}

pub const PASTE_TOOL: usize = 0;
pub const FIRST_TOOL: usize = 1;
pub const BRUSH_TOOL: usize = 4;
pub const PIPETTE_TOOL: usize = 6;

impl<'a> MainWindow<'a> {
    pub fn create_id(&mut self) -> usize {
        self.id += 1;
        self.id
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fnt = crate::model::font_imp::FontTool {
            selected_font: Arc::new(Mutex::new(0)),
            fonts: Arc::new(Mutex::new(Vec::new())),
            sizes: Vec::new(),
            prev_char: ' ',
        };
        fnt.load_fonts();
        fnt.install_watcher();
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let tools: Vec<Box<dyn Tool>> = vec![
            Box::<crate::model::paste_tool::PasteTool>::default(),
            Box::<crate::model::click_imp::ClickTool>::default(),
            Box::<crate::model::select_imp::SelectTool>::default(),
            Box::<crate::model::pencil_imp::PencilTool>::default(),
            Box::<crate::model::brush_imp::BrushTool>::default(),
            Box::<crate::model::erase_imp::EraseTool>::default(),
            Box::<crate::model::pipette_imp::PipetteTool>::default(),
            Box::<crate::model::line_imp::LineTool>::default(),
            Box::<crate::model::flip_imp::FlipTool>::default(),
            Box::<crate::model::draw_rectangle_imp::DrawRectangleTool>::default(),
            Box::<crate::model::draw_rectangle_filled_imp::DrawRectangleFilledTool>::default(),
            Box::<crate::model::draw_ellipse_imp::DrawEllipseTool>::default(),
            Box::<crate::model::draw_ellipse_filled_imp::DrawEllipseFilledTool>::default(),
            Box::new(crate::model::fill_imp::FillTool::new()),
            Box::new(fnt),
            Box::<crate::model::move_layer_imp::MoveLayer>::default(),
            Box::<crate::model::tag_imp::TagTool>::default(),
        ];

        let ctx: &egui::Context = &cc.egui_ctx;

        let mut style: egui::Style = (*ctx.style()).clone();
        style.spacing.window_margin = egui::Margin::same(8.0);
        use egui::FontFamily::Proportional;
        use egui::TextStyle::{Body, Button, Heading, Monospace, Small};
        style.text_styles = [
            (Heading, FontId::new(24.0, Proportional)),
            (Body, FontId::new(18.0, Proportional)),
            (Monospace, FontId::new(18.0, egui::FontFamily::Monospace)),
            (Button, FontId::new(18.0, Proportional)),
            (Small, FontId::new(14.0, Proportional)),
        ]
        .into();
        ctx.set_style(style);

        let gl = cc.gl.clone().unwrap();

        let mut tool_tree = egui_tiles::Tree::<ToolTab>::empty("tool_tree");
        let layers = tool_tree.tiles.insert_pane(ToolTab::new(LayerToolWindow::new(gl.clone())));
        let channels = tool_tree.tiles.insert_pane(ToolTab::new(ChannelToolWindow::default()));
        let minimap = tool_tree.tiles.insert_pane(ToolTab::new(MinimapToolWindow::new(gl.clone())));

        let tab = tool_tree.tiles.insert_tab_tile(vec![minimap]);
        let tab2 = tool_tree.tiles.insert_tab_tile(vec![layers, channels]);
        let vert_id = tool_tree.tiles.insert_vertical_tile(vec![tab, tab2]);
        if let Some(egui_tiles::Tile::Container(Container::Linear(linear))) = tool_tree.tiles.get_mut(vert_id) {
            linear.shares.set_share(tab, 3.0);
            linear.shares.set_share(tab2, 1.25);
        }

        tool_tree.root = Some(vert_id);
        let open_file_window = icy_view_gui::MainWindow::new(&gl, None, icy_view_gui::options::Options::default());
        let settings_dialog = SettingsDialog::new(ctx, &gl);

        let mut key_bindings = KeyBindings::default();

        if let Ok(settings_file) = KeyBindings::get_keybindings_file() {
            if settings_file.exists() {
                if let Ok(bindings) = KeyBindings::load(&settings_file) {
                    key_bindings = bindings;
                }
            }
        }

        let mut c = Box::<Commands>::default();
        c.apply_key_bindings(&key_bindings.key_bindings);
        let plugins = Plugin::read_plugin_directory();

        let mut mru_files = MostRecentlyUsedFiles::default();
        if let Ok(mru_file_path) = MostRecentlyUsedFiles::get_mru_file() {
            if mru_file_path.exists() {
                if let Ok(mru) = MostRecentlyUsedFiles::load(&mru_file_path) {
                    mru_files = mru;
                }
            }
        }
        ctx.set_theme(unsafe { SETTINGS.get_theme() });
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
            o.zoom_factor = 1.0;
        });

        MainWindow {
            document_behavior: DocumentBehavior::new(Arc::new(Mutex::new(tools))),
            tool_behavior: ToolBehavior::default(),
            toasts: egui_notify::Toasts::default(),
            document_tree: egui_tiles::Tree::<DocumentTab>::empty("document_tree"),
            tool_tree,
            gl,
            dialog_open: false,
            modal_dialog: None,
            id: 0,
            left_panel: true,
            right_panel: true,
            bottom_panel: false,
            top_bar: TopBar::new(&cc.egui_ctx),
            commands: vec![c],
            request_close: false,
            allowed_to_close: false,
            close_all_requested: false,
            is_fullscreen: false,
            set_fullscreen_opt: None,
            in_open_file_mode: false,
            open_file_window,
            show_settings: false,
            settings_dialog,
            last_command_update: Instant::now(),
            current_id: None,
            title: String::new(),
            key_bindings,
            plugins,
            mru_files,
        }
    }

    pub fn open_data(&mut self, full_path: Option<PathBuf>, file_name: &Path, data: &[u8], terminal_width: Option<usize>) {
        if let Some(full_path) = &full_path {
            self.mru_files.add_recent_file(&full_path);
            if let Some(ext) = full_path.extension() {
                let ext = ext.to_str().unwrap_or_default().to_ascii_lowercase();
                if is_font_extensions(&ext) {
                    let file_name = full_path.file_name();
                    if file_name.is_none() {
                        return;
                    }

                    if ext == "yaff" {
                        if let Ok(txt) = fs::read_to_string(&full_path) {
                            if let Ok(font) = BitFont::from_str(&txt) {
                                add_child(&mut self.document_tree, Some(full_path.clone()), Box::new(BitFontEditor::new(&self.gl, font)));
                                return;
                            }
                        }
                    } else {
                        let file_name_str = file_name.unwrap_or_default().to_str().unwrap_or_default().to_string();
                        if let Ok(font) = BitFont::from_bytes(file_name_str, data) {
                            add_child(&mut self.document_tree, Some(full_path.clone()), Box::new(BitFontEditor::new(&self.gl, font)));
                            return;
                        }
                    }
                }

                if "icyanim" == ext {
                    match std::str::from_utf8(data) {
                        Ok(txt) => {
                            add_child(
                                &mut self.document_tree,
                                Some(full_path.clone()),
                                Box::new(crate::AnimationEditor::new(self.gl.clone(), full_path, txt.to_string())),
                            );
                        }
                        Err(err) => {
                            self.show_error(format!("{err}"));
                        }
                    }
                    return;
                }

                if "tdf" == ext {
                    let file_name = full_path.file_name();
                    if file_name.is_none() {
                        return;
                    }
                    if let Ok(fonts) = TheDrawFont::from_bytes(data) {
                        add_child(&mut self.document_tree, Some(full_path.clone()), Box::new(CharFontEditor::new(&self.gl, fonts)));
                        return;
                    }
                }
            }
        }
        match Buffer::from_bytes(file_name, true, data, None, terminal_width) {
            Ok(mut buf) => {
                buf.is_terminal_buffer = false;
                let editor = AnsiEditor::new(&self.gl, buf);
                add_child(&mut self.document_tree, full_path, Box::new(editor));
            }
            Err(err) => {
                log::error!("Error loading file: {}", err);
                self.toasts.error(fl!(crate::LANGUAGE_LOADER, "error-load-file", error = err.to_string()));
                //.set_duration(Some(Duration::from_secs(5)));
            }
        }
    }

    pub fn open_file(&mut self, path: &Path, load_autosave: bool, terminal_width: Option<usize>) {
        let mut already_open = None;
        self.enumerate_documents(|id, pane| {
            if let Some(doc_path) = pane.get_path() {
                if doc_path == *path {
                    already_open = Some(id);
                }
            }
        });

        if let Some(id) = already_open {
            self.enumerate_tabs(|_, tab| {
                if tab.children.contains(&id) {
                    tab.active = Some(id);
                }
            });
            return;
        }
        let load_path = if load_autosave {
            autosave::get_autosave_file(path)
        } else {
            path.to_path_buf()
        };

        match fs::read(load_path) {
            Ok(data) => {
                self.open_data(
                    Some(path.to_path_buf()),
                    &PathBuf::from(path.file_name().unwrap_or_default()),
                    &data,
                    terminal_width,
                );
            }
            Err(err) => {
                log::error!("error loading file {path:?}: {err}");
                self.toasts.error(format!("{err}")); //.set_duration(Some(Duration::from_secs(5)));
            }
        }
    }

    pub fn get_active_pane_mut(&mut self) -> Option<&mut DocumentTab> {
        let mut stack = vec![];

        if let Some(root) = self.document_tree.root {
            stack.push(root);
        }
        while let Some(id) = stack.pop() {
            match self.document_tree.tiles.get(id) {
                Some(egui_tiles::Tile::Pane(_)) => {
                    if let Some(egui_tiles::Tile::Pane(p)) = self.document_tree.tiles.get_mut(id) {
                        return Some(p);
                    } else {
                        return None;
                    }
                }
                Some(egui_tiles::Tile::Container(container)) => match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        if let Some(active) = tabs.active {
                            stack.push(active);
                        }
                    }
                    egui_tiles::Container::Linear(l) => {
                        for child in l.children.iter() {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Grid(g) => {
                        for child in g.children() {
                            stack.push(*child);
                        }
                    }
                },
                None => {}
            }
        }

        None
    }

    pub fn get_active_pane(&mut self) -> Option<(TileId, &DocumentTab)> {
        let mut stack = vec![];

        if let Some(root) = self.document_tree.root {
            stack.push(root);
        }
        while let Some(id) = stack.pop() {
            match self.document_tree.tiles.get(id) {
                Some(egui_tiles::Tile::Pane(_)) => {
                    if let Some(egui_tiles::Tile::Pane(p)) = self.document_tree.tiles.get(id) {
                        return Some((id, p));
                    } else {
                        return None;
                    }
                }
                Some(egui_tiles::Tile::Container(container)) => match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        if let Some(active) = tabs.active {
                            stack.push(active);
                        }
                    }
                    egui_tiles::Container::Linear(l) => {
                        for child in l.children.iter() {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Grid(g) => {
                        for child in g.children() {
                            stack.push(*child);
                        }
                    }
                },
                None => {}
            }
        }

        None
    }

    pub fn enumerate_documents<F>(&mut self, mut callback: F)
    where
        F: FnMut(TileId, &mut DocumentTab),
    {
        let mut stack = vec![];

        if let Some(root) = self.document_tree.root {
            stack.push(root);
        }
        while let Some(id) = stack.pop() {
            match self.document_tree.tiles.get(id) {
                Some(egui_tiles::Tile::Pane(_)) => {
                    if let Some(egui_tiles::Tile::Pane(p)) = self.document_tree.tiles.get_mut(id) {
                        callback(id, p);
                    }
                }
                Some(egui_tiles::Tile::Container(container)) => match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        for child in &tabs.children {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Linear(l) => {
                        for child in l.children.iter() {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Grid(g) => {
                        for child in g.children() {
                            stack.push(*child);
                        }
                    }
                },
                None => {}
            }
        }
    }

    pub fn enumerate_tabs<F>(&mut self, mut callback: F)
    where
        F: FnMut(TileId, &mut egui_tiles::Tabs),
    {
        let mut stack = vec![];

        if let Some(root) = self.document_tree.root {
            stack.push(root);
        }
        while let Some(id) = stack.pop() {
            match self.document_tree.tiles.get_mut(id) {
                Some(egui_tiles::Tile::Pane(_)) => {}
                Some(egui_tiles::Tile::Container(container)) => match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        callback(id, tabs);

                        for child in &tabs.children {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Linear(l) => {
                        for child in l.children.iter() {
                            stack.push(*child);
                        }
                    }
                    egui_tiles::Container::Grid(g) => {
                        for child in g.children() {
                            stack.push(*child);
                        }
                    }
                },
                None => {}
            }
        }
    }

    pub fn get_active_document(&mut self) -> Option<Arc<Mutex<Box<dyn Document>>>> {
        if let Some(pane) = self.get_active_pane_mut() {
            return Some(pane.doc.clone());
        }
        None
    }

    pub(crate) fn open_dialog<T: ModalDialog + 'static>(&mut self, dialog: T) {
        self.modal_dialog = Some(Box::new(dialog));
    }

    pub(crate) fn run_editor_command<T>(&mut self, param: T, func: impl Fn(&mut MainWindow<'_>, &mut AnsiEditor, T) -> Option<Message>) {
        let mut msg = None;
        if let Some(doc) = self.get_active_document() {
            if let Some(editor) = doc.lock().get_ansi_editor_mut() {
                msg = func(self, editor, param);
            }
        }
        self.handle_message(msg);
    }

    pub fn show_error(&mut self, str: String) {
        log::error!("Error: {str}");
        self.toasts.error(fl!(crate::LANGUAGE_LOADER, "error-load-file", error = str));
        //.set_duration(Some(Duration::from_secs(5)));
    }

    pub(crate) fn handle_result<T>(&mut self, result: EngineResult<T>) -> Option<T> {
        match result {
            Err(err) => {
                self.show_error(format!("{err}"));
                None
            }
            Ok(res) => Some(res),
        }
    }

    fn request_close_tab(&mut self, close_id: TileId) -> bool {
        let mut result = true;
        let mut msg = None;
        if let Some(egui_tiles::Tile::Pane(pane)) = self.document_tree.tiles.get(close_id) {
            if !pane.is_dirty() {
                if let Some(egui_tiles::Tile::Pane(pane)) = self.document_tree.tiles.get_mut(close_id) {
                    msg = pane.destroy(&self.gl);
                }
                self.document_tree.tiles.remove(close_id);
            } else {
                self.open_dialog(AskCloseFileDialog::new(pane.get_path(), close_id));
                result = false;
            }
        }
        self.handle_message(msg);
        result
    }

    pub fn update_title(&mut self, ctx: &egui::Context, force: bool) {
        if !force {
            let id = if let Some((id, _)) = self.get_active_pane() { Some(id) } else { None };
            if let Some(id) = id {
                if self.current_id == Some(id) {
                    return;
                }
                self.current_id = Some(id);
            } else {
                if self.current_id.is_some() {
                    self.current_id = None;
                    self.set_title(ctx, crate::DEFAULT_TITLE.clone());
                }
                return;
            }
        }

        if let Some((_, doc)) = self.get_active_pane() {
            if let Some(path) = doc.get_path() {
                let title = if let Some(parent) = path.parent() {
                    let directory = crate::util::shorten_directory(parent);
                    format!(
                        "{}{} - iCY DRAW {}",
                        directory,
                        path.file_name().unwrap_or_default().to_str().unwrap_or_default(),
                        *crate::VERSION
                    )
                } else {
                    format!(
                        "{} - iCY DRAW {}",
                        path.file_name().unwrap_or_default().to_str().unwrap_or_default(),
                        *crate::VERSION
                    )
                };
                self.set_title(ctx, title);
            }
        }
    }

    fn set_title(&mut self, ctx: &egui::Context, title: String) {
        if self.title != title {
            self.title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }
}

pub fn is_font_extensions(ext: &str) -> bool {
    "psf" == ext
        || "f19" == ext
        || "f18" == ext
        || "f17" == ext
        || "f16" == ext
        || "f15" == ext
        || "f14" == ext
        || "f13" == ext
        || "f12" == ext
        || "f11" == ext
        || "f10" == ext
        || "f09" == ext
        || "f08" == ext
        || "f07" == ext
        || "f06" == ext
        || "f05" == ext
        || "f04" == ext
        || "f03" == ext
        || "f02" == ext
        || "f01" == ext
        || "fon" == ext
        || "yaff" == ext
}

pub fn button_with_shortcut(ui: &mut Ui, enabled: bool, label: impl Into<String>, shortcut: impl Into<String>) -> Response {
    let title = label.into();
    let button = Button::new(title).shortcut_text(shortcut.into());
    ui.add_enabled(enabled, button)
}

impl<'a> eframe::App for MainWindow<'a> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let focus = ctx.memory(|r| r.focused());

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.allowed_to_close {
                // do nothing - we will close
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.request_close = true;
                self.close_all_requested = false;
            }
        }

        if self.request_close {
            let mut dirty_files = Vec::new();
            let mut ids = Vec::new();
            for tile in self.document_tree.active_tiles().iter() {
                if let Some(egui_tiles::Tile::Pane(p)) = self.document_tree.tiles.get(*tile) {
                    if p.is_dirty() {
                        ids.push(*tile);
                        dirty_files.push(p.get_path().unwrap_or_default());
                    }
                }
            }
            if dirty_files.len() > 0 {
                self.open_dialog(super::UnsavedFilesDialog::new(dirty_files, ids));
                self.request_close = false;
                self.allowed_to_close = false;
            } else {
                self.request_close = false;
                self.allowed_to_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        ctx.input_mut(|i| {
            for e in &mut i.events {
                match e {
                    egui::Event::Copy => {
                        *e = egui::Event::Key {
                            key: Key::C,
                            modifiers: Modifiers::CTRL,
                            physical_key: None,
                            repeat: false,
                            pressed: true,
                        };
                    }
                    egui::Event::Cut => {
                        *e = egui::Event::Key {
                            key: Key::X,
                            modifiers: Modifiers::CTRL,
                            physical_key: None,
                            repeat: false,
                            pressed: true,
                        };
                    }
                    egui::Event::Paste(_) => {
                        *e = egui::Event::Key {
                            key: Key::V,
                            modifiers: Modifiers::CTRL,
                            physical_key: None,
                            repeat: false,
                            pressed: true,
                        };
                    }
                    _ => {}
                }
            }
        });

        if self.in_open_file_mode {
            if self.open_file_window.show_file_chooser(ctx, unsafe { SETTINGS.monitor_settings.clone() }) {
                let file: usize = self.open_file_window.opened_file.take().unwrap_or_default();

                if !self.open_file_window.is_canceled && !self.open_file_window.file_view.files[file].is_folder() {
                    let path = self.open_file_window.file_view.files[file].get_file_path();
                    if self.open_file_window.file_view.files[file].is_virtual_file() {
                        if let Some(data) = self.open_file_window.file_view.files[file].read_data() {
                            self.open_data(
                                None,
                                &PathBuf::from(path.file_name().unwrap()),
                                &data,
                                Some(self.open_file_window.get_terminal_width()),
                            );
                        }
                    } else {
                        self.open_file(&path, false, Some(self.open_file_window.get_terminal_width()));
                    }

                    /*/
                    if file.file_info.path.exists() {
                    } else if let Some(data) = &file.file_data {
                        let mut path = file.file_info.path.clone();
                        if let Some(user) = UserDirs::new() {
                            if let Some(dir) = user.document_dir() {
                                path = dir.join(path);
                                while path.exists() {
                                    path = path.with_extension(format!("1.{}", file.file_info.path.extension().unwrap().to_string_lossy()));
                                }
                            }
                        }
                        self.open_data(&path, data);
                    }*/
                }
                self.in_open_file_mode = false;
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.in_open_file_mode = false;
            }
            return;
        }

        let msg = self.show_top_bar(ctx, frame);
        self.handle_message(msg);

        egui::TopBottomPanel::bottom("status_bar_panel")
            .frame(egui::Frame {
                fill: ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.allocate_ui_with_layout(ui.available_rect_before_wrap().size(), Layout::left_to_right(egui::Align::Min), |ui| {
                    let draw_rect = ui.available_rect_before_wrap();
                    let font_id = TextStyle::Body.resolve(ui.style());

                    let text: WidgetText = if let Some(doc) = self.get_active_document() {
                        if let Some(editor) = doc.lock().get_ansi_editor() {
                            let ice_mode = editor.buffer_view.lock().get_buffer().ice_mode;

                            let ice_mode_text = match ice_mode {
                                icy_engine::IceMode::Unlimited => "~",
                                icy_engine::IceMode::Blink => "B",
                                icy_engine::IceMode::Ice => "I",
                            };

                            let pal_mode = editor.buffer_view.lock().get_buffer().palette_mode;
                            let pal_mode_text = match pal_mode {
                                icy_engine::PaletteMode::RGB => "RGB",
                                icy_engine::PaletteMode::Fixed16 => "DOS",
                                icy_engine::PaletteMode::Free8 => "8",
                                icy_engine::PaletteMode::Free16 => "16",
                            };
                            format!("[{:03}%] [{ice_mode_text}-{pal_mode_text}]", (100. * unsafe { SETTINGS.get_scale().x }) as i32)
                        } else {
                            format!("[{:03}%]", (100. * unsafe { SETTINGS.get_scale().x }) as i32)
                        }
                    } else {
                        format!("[{:03}%]", (100. * unsafe { SETTINGS.get_scale().x }) as i32)
                    }
                    .into();
                    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id.clone());
                    let rect = Rect::from_min_size(
                        Pos2::new(draw_rect.left() + 8.0, draw_rect.top() + (draw_rect.height() - galley.size().y) / 2.0),
                        galley.size(),
                    );
                    ui.painter().galley_with_override_text_color(
                        egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min,
                        galley,
                        ui.style().visuals.text_color(),
                    );
                    let cur_x = rect.right();

                    if let Some(doc) = self.get_active_document() {
                        if let Some(editor) = doc.lock().get_ansi_editor() {
                            let font = editor.buffer_view.lock().get_edit_state().get_caret().get_font_page();
                            let use_letter_spacing = editor.buffer_view.lock().get_buffer().use_letter_spacing();
                            let use_ar = editor.buffer_view.lock().get_buffer().use_aspect_ratio();
                            if let Some(font) = editor.buffer_view.lock().get_buffer().get_font(font) {
                                let text: WidgetText = format!(
                                    "{} - {}x{} {} {}",
                                    font.name,
                                    font.size.width,
                                    font.size.height,
                                    if use_letter_spacing { "(9px)" } else { "" },
                                    if use_ar { "(ar)" } else { "" },
                                )
                                .into();
                                let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id.clone());
                                let rect = Rect::from_min_size(
                                    Pos2::new(cur_x + 8.0, draw_rect.top() + (draw_rect.height() - galley.size().y) / 2.0),
                                    galley.size(),
                                );
                                ui.painter().galley_with_override_text_color(
                                    egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min,
                                    galley,
                                    ui.style().visuals.text_color(),
                                );
                            }

                            // center text
                            let size = editor.buffer_view.lock().get_buffer().get_size();
                            let text: WidgetText = if let Some(sel) = editor.buffer_view.lock().get_selection() {
                                let r = sel.as_rectangle();
                                fl!(crate::LANGUAGE_LOADER, "toolbar-size", colums = r.get_width(), rows = r.get_height())
                            } else {
                                fl!(crate::LANGUAGE_LOADER, "toolbar-size", colums = size.width, rows = size.height)
                            }
                            .into();

                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id.clone());
                            let rect = Rect::from_min_size(draw_rect.center() - galley.size() / 2.0, galley.size());
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min,
                                galley,
                                ui.style().visuals.text_color(),
                            );

                            let pos: icy_engine::Position = editor.buffer_view.lock().get_caret().get_position();
                            let insert_mode = editor.buffer_view.lock().get_caret().insert_mode;
                            let text: WidgetText = format!(
                                "{} {}",
                                if insert_mode { "(Ins) " } else { "" },
                                fl!(crate::LANGUAGE_LOADER, "toolbar-position", line = (pos.y + 1), column = (pos.x + 1))
                            )
                            .into();

                            let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id.clone());
                            let rect = Rect::from_min_size(
                                Pos2::new(
                                    draw_rect.right() - 8.0 - galley.size().x,
                                    draw_rect.top() + (draw_rect.height() - galley.size().y) / 2.0,
                                ),
                                galley.size(),
                            );
                            ui.painter().galley_with_override_text_color(
                                egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min,
                                galley,
                                ui.style().visuals.text_color(),
                            );
                        }
                    }
                });
            });

        SidePanel::left("left_panel")
            .exact_width(264.0)
            .resizable(false)
            .frame(egui::Frame {
                fill: ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show_animated(ctx, self.left_panel, |ui| {
                ui.add_space(8.0);
                let mut msg = None;

                let mut caret_attr = TextAttribute::default();
                let mut palette = Palette::dos_default();
                let mut ice_mode = icy_engine::IceMode::Unlimited;
                let mut font_mode = icy_engine::FontMode::Unlimited;
                let mut buffer_type = BufferType::CP437;

                if let Some(doc) = self.get_active_document() {
                    if let Some(editor) = doc.lock().get_ansi_editor() {
                        buffer_type = editor.buffer_view.lock().get_buffer().buffer_type;
                        caret_attr = editor.buffer_view.lock().get_caret().get_attribute();
                        palette = editor.buffer_view.lock().get_buffer().palette.clone();
                        ice_mode = editor.buffer_view.lock().get_buffer().ice_mode;
                        font_mode = editor.buffer_view.lock().get_buffer().font_mode;
                    }
                }

                /*
                   let caret_attr = editor.buffer_view.lock().get_caret().get_attribute();
                   let palette = editor.buffer_view.lock().get_buffer().palette.clone();
                */
                ui.vertical_centered(|ui| {
                    msg = crate::palette_switcher(ctx, ui, &caret_attr, &palette);
                });

                ui.separator();

                let is_atari = matches!(buffer_type, BufferType::Atascii);

                if !is_atari {
                    let msg2 = crate::palette_editor_16(ui, &caret_attr, &palette, ice_mode, font_mode);
                    if msg.is_none() {
                        msg = msg2;
                    }

                    if ice_mode.has_blink()
                        && ui
                            .selectable_label(caret_attr.is_blinking(), fl!(crate::LANGUAGE_LOADER, "color-is_blinking"))
                            .clicked()
                    {
                        if let Some(doc) = self.get_active_document() {
                            if let Some(editor) = doc.lock().get_ansi_editor() {
                                caret_attr.set_is_blinking(!caret_attr.is_blinking());
                                editor.buffer_view.lock().get_caret_mut().set_attr(caret_attr);
                            }
                        }
                    }

                    ui.separator();
                }

                self.handle_message(msg);

                let msg = crate::add_tool_switcher(ctx, ui, self);
                self.handle_message(msg);

                let mut tool_result = None;
                if let Some(tool) = self.document_behavior.tools.clone().lock().get_mut(self.document_behavior.get_selected_tool()) {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            if let Some(doc) = self.get_active_document() {
                                let mut shown = false;
                                if let Some(editor) = doc.lock().get_ansi_editor_mut() {
                                    shown = true;
                                    tool_result = tool.show_ui(ctx, ui, Some(editor))
                                }
                                if !shown {
                                    tool_result = tool.show_doc_ui(ctx, ui, doc.clone());
                                }
                            }
                        });
                    });
                }
                // can't handle message inside the lock
                self.handle_message(tool_result);
            });

        let panel_frame = egui::Frame {
            fill: ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::SidePanel::right("right_panel")
            .frame(panel_frame)
            .resizable(false)
            .show_animated(ctx, self.right_panel, |ui| {
                self.tool_behavior.active_document = self.get_active_document();
                self.tool_tree.ui(&mut self.tool_behavior, ui);
                self.tool_behavior.active_document = None;
                let msg = self.tool_behavior.message.take();
                self.handle_message(msg);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: ctx.style().visuals.panel_fill,
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.set_width((ui.available_width() - 250.0).max(0.0));
                self.document_tree.ui(&mut self.document_behavior, ui);

                if self.document_behavior.get_selected_tool() != PASTE_TOOL {
                    if let Some(doc) = self.get_active_document() {
                        if let Some(editor) = doc.lock().get_ansi_editor() {
                            let lock = &mut editor.buffer_view.lock();
                            let paste_mode = lock.get_buffer().layers.iter().position(|layer| layer.role.is_paste());
                            if let Some(layer) = paste_mode {
                                self.document_behavior.tools.lock()[PASTE_TOOL] =
                                    Box::new(crate::model::paste_tool::PasteTool::new(self.document_behavior.get_selected_tool()));
                                self.document_behavior.set_selected_tool(PASTE_TOOL);
                                lock.get_edit_state_mut().set_current_layer(layer);
                            }
                        }
                    }
                }
            });

        self.dialog_open = false;
        let mut dialog_message = None;
        if self.modal_dialog.is_some() {
            self.dialog_open = true;
            if self.modal_dialog.as_mut().unwrap().show(ctx) {
                if let Some(modal_dialog) = self.modal_dialog.take() {
                    if modal_dialog.should_commit() {
                        if let Some(doc) = self.get_active_document() {
                            if let Some(editor) = doc.lock().get_ansi_editor_mut() {
                                match modal_dialog.commit(editor) {
                                    Ok(msg) => {
                                        dialog_message = msg;
                                    }
                                    Err(err) => {
                                        log::error!("Error: {}", err);
                                        self.toasts.error(format!("{err}")); //.set_duration(Some(Duration::from_secs(5)));
                                    }
                                }
                            }
                        }
                        match modal_dialog.commit_self(ctx, self) {
                            Ok(msg) => {
                                if dialog_message.is_none() {
                                    dialog_message = msg;
                                }
                            }
                            Err(err) => {
                                log::error!("Error: {}", err);
                                self.toasts.error(format!("{err}")); //.set_duration(Some(Duration::from_secs(5)));
                            }
                        }
                    }
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.modal_dialog = None;
            }
        }
        self.handle_message(dialog_message);

        self.toasts.show(ctx);
        if let Some(close_id) = self.document_behavior.request_close.take() {
            self.request_close_tab(close_id);
        }

        if let Some(close_id) = self.document_behavior.request_close_all.take() {
            let mut open_tab = Vec::new();
            self.enumerate_tabs(|_, tab| {
                if tab.children.contains(&close_id) {
                    open_tab = tab.children.clone();
                }
            });
            for t in open_tab {
                if !self.request_close_tab(t) {
                    break;
                }
            }
        }

        if let Some(close_id) = self.document_behavior.request_close_others.take() {
            let mut open_tab = Vec::new();
            self.enumerate_tabs(|_, tab| {
                if tab.children.contains(&close_id) {
                    open_tab = tab.children.clone();
                }
            });
            for t in open_tab {
                if t != close_id && !self.request_close_tab(t) {
                    break;
                }
            }
        }

        let mut msg = self.document_behavior.message.take();
        self.commands[0].check(ctx, &mut msg);
        self.handle_message(msg);
        self.handle_message(read_outline_keys(ctx));
        self.handle_message(read_color_keys(ctx));
        let mut force_update_title = false;

        ctx.input(|i| {
            for f in &i.raw.dropped_files {
                if let Some(path) = &f.path {
                    self.open_file(path, false, None);
                }
            }
            for evt in &i.events.clone() {
                match evt {
                    eframe::egui::Event::PointerButton {
                        button: PointerButton::Middle,
                        pressed: true,
                        ..
                    } => {
                        self.handle_message(Some(Message::SelectTool(PIPETTE_TOOL)));
                    }
                    eframe::egui::Event::Zoom(vec) => {
                        let scale = unsafe { SETTINGS.get_scale() } * *vec;
                        unsafe {
                            SETTINGS.set_scale(scale);
                            force_update_title = true;
                        }
                    }
                    egui::Event::MouseWheel { delta, modifiers, .. } => {
                        if modifiers.ctrl || modifiers.mac_cmd {
                            let scale = unsafe { SETTINGS.get_scale() } + Vec2::splat(delta.y) * 0.1;
                            unsafe {
                                SETTINGS.set_scale(scale);
                                force_update_title = true;
                            }
                        }
                    }
                    _ => (),
                }
            }
        });

        if let Some(fullscreen) = self.set_fullscreen_opt.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }

        if self.show_settings {
            self.show_settings = self.settings_dialog.show(ctx);
            if !self.show_settings {
                unsafe { SETTINGS.is_dark_mode = self.settings_dialog.is_dark_mode };
                if self.key_bindings.key_bindings != self.settings_dialog.key_bindings.key_bindings {
                    self.key_bindings = self.settings_dialog.key_bindings.clone();
                    if let Err(err) = self.key_bindings.save() {
                        log::error!("Error saving keybindings: {}", err);
                    }
                    self.commands[0].apply_key_bindings(&self.key_bindings.key_bindings);
                }

                if let Err(err) = super::Settings::save() {
                    log::error!("Error saving settings: {}", err);
                }
            }
        }

        if let Some(id) = focus {
            if ctx.memory(|r| r.focused()).is_none() {
                ctx.memory_mut(|r| {
                    r.request_focus(id);
                });
            }
        }
        self.update_title(ctx, force_update_title);
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn read_outline_keys(ctx: &egui::Context) -> Option<Message> {
    let mut result = None;

    if ctx.input(|i| i.key_pressed(Key::F1) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(0));
    }
    if ctx.input(|i| i.key_pressed(Key::F2) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(1));
    }
    if ctx.input(|i| i.key_pressed(Key::F3) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(2));
    }
    if ctx.input(|i| i.key_pressed(Key::F4) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(3));
    }
    if ctx.input(|i| i.key_pressed(Key::F5) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(4));
    }
    if ctx.input(|i| i.key_pressed(Key::F6) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(5));
    }
    if ctx.input(|i| i.key_pressed(Key::F7) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(6));
    }
    if ctx.input(|i| i.key_pressed(Key::F8) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(7));
    }
    if ctx.input(|i| i.key_pressed(Key::F9) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(8));
    }
    if ctx.input(|i| i.key_pressed(Key::F10) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(9));
    }
    if ctx.input(|i| i.key_pressed(Key::F11) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(10));
    }
    if ctx.input(|i| i.key_pressed(Key::F12) && check_base_f_key_modifier(i)) {
        result = Some(Message::SelectOutline(11));
    }
    if ctx.input(|i| i.key_pressed(Key::F1) && i.modifiers.shift && (i.modifiers.ctrl || i.modifiers.alt)) {
        result = Some(Message::SelectOutline(12));
    }
    if ctx.input(|i| i.key_pressed(Key::F2) && i.modifiers.shift && (i.modifiers.ctrl || i.modifiers.alt)) {
        result = Some(Message::SelectOutline(13));
    }
    if ctx.input(|i| i.key_pressed(Key::F3) && i.modifiers.shift && (i.modifiers.ctrl || i.modifiers.alt)) {
        result = Some(Message::SelectOutline(14));
    }

    result
}

fn read_color_keys(ctx: &egui::Context) -> Option<Message> {
    let mut result = None;

    let keys = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7];

    for (i, k) in keys.iter().enumerate() {
        if ctx.input(|i| i.key_pressed(*k) && i.modifiers.command_only()) {
            result = Some(Message::KeySwitchForeground(i));
        }
        if ctx.input(|i| i.key_pressed(*k) && i.modifiers.alt && !i.modifiers.shift && !i.modifiers.ctrl) {
            result = Some(Message::KeySwitchBackground(i));
        }
    }
    result
}

fn check_base_f_key_modifier(i: &egui::InputState) -> bool {
    i.modifiers.command_only() || (i.modifiers.alt && !i.modifiers.shift && !i.modifiers.ctrl)
}
