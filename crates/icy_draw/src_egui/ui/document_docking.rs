use std::{fs, path::PathBuf, sync::Arc, time::Instant};

use crate::{
    DEFAULT_CHAR_SET_TABLE, Document, DocumentOptions, FIRST_TOOL, Message, Settings, create_image,
    model::Tool,
    util::autosave::{remove_autosave, store_auto_save},
};
use eframe::{
    egui::{self, Response, Ui},
    epaint::Rgba,
};
use egui::{Sense, TextureHandle, Widget, mutex::Mutex};
use egui_tiles::{Tabs, TileId, Tiles};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, Buffer, TextAttribute, TextPane};

pub struct DocumentTab {
    full_path: Option<PathBuf>,
    pub doc: Arc<Mutex<Box<dyn Document>>>,
    last_save: usize,

    // autosave variables
    auto_save_status: usize,
    instant: Instant,
    last_change_autosave_timer: usize,
    destroyed: bool,
}
impl DocumentTab {
    pub fn is_dirty(&self) -> bool {
        let undo_stack_len = self.doc.lock().undo_stack_len();
        self.last_save != undo_stack_len
    }

    pub(crate) fn save(&mut self) -> Option<Message> {
        let Some(path) = &self.full_path else {
            log::error!("No path to save to");
            return None;
        };
        let doc = &mut self.doc.lock();

        let mut msg = None;
        match doc.get_bytes(path) {
            Ok(bytes) => {
                let mut tmp_file = path.clone();
                let ext = path.extension().unwrap_or_default().to_str().unwrap_or_default().to_ascii_lowercase();

                tmp_file.with_extension(format!("{}~", ext));
                let mut num = 1;
                while tmp_file.exists() {
                    tmp_file = tmp_file.with_extension(format!("{}{}~", ext, num));
                    num += 1;
                }

                if let Err(err) = fs::write(&tmp_file, bytes) {
                    msg = Some(Message::ShowError(format!("Error writing file {err}")));
                } else if let Err(err) = fs::rename(tmp_file, path) {
                    msg = Some(Message::ShowError(format!("Error moving file {err}")));
                }
                remove_autosave(path);

                let undo_stack_len = doc.undo_stack_len();
                self.last_save = undo_stack_len;
                self.last_change_autosave_timer = undo_stack_len;
                self.auto_save_status = undo_stack_len;
                doc.inform_save();
            }
            Err(err) => {
                msg = Some(Message::ShowError(format!("{err}")));
            }
        }
        if msg.is_none() {
            remove_autosave(path);
        }
        msg
    }

    pub fn get_path(&self) -> Option<PathBuf> {
        self.full_path.clone()
    }

    pub fn set_path(&mut self, mut path: PathBuf) {
        let doc = &mut self.doc.lock();
        path.set_extension(doc.default_extension());
        if let Some(old_path) = &self.full_path {
            remove_autosave(old_path);
        }
        self.full_path = Some(path);
    }

    pub fn is_untitled(&self) -> bool {
        self.full_path.is_none()
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    pub fn destroy(&mut self, gl: &glow::Context) -> Option<Message> {
        if self.destroyed {
            return None;
        }
        self.destroyed = true;
        self.doc.lock().destroy(gl)
    }
}

pub struct DocumentBehavior {
    pub tools: Arc<Mutex<Vec<Box<dyn Tool>>>>,
    selected_tool: usize,
    prev_tool: usize,
    pub document_options: DocumentOptions,

    char_set_img: Option<TextureHandle>,
    cur_char_set: usize,
    dark_mode: bool,

    pos_img: Option<TextureHandle>,

    pub request_close: Option<TileId>,
    pub request_close_others: Option<TileId>,
    pub request_close_all: Option<TileId>,

    pub message: Option<Message>,
}

impl DocumentBehavior {
    pub fn new(tools: Arc<Mutex<Vec<Box<dyn Tool>>>>) -> Self {
        Self {
            tools,
            selected_tool: FIRST_TOOL,
            prev_tool: FIRST_TOOL,
            document_options: DocumentOptions::default(),
            char_set_img: None,
            cur_char_set: usize::MAX,
            request_close: None,
            request_close_others: None,
            request_close_all: None,
            message: None,
            pos_img: None,
            dark_mode: true,
        }
    }

    pub fn get_selected_tool(&self) -> usize {
        self.selected_tool
    }

    pub(crate) fn set_selected_tool(&mut self, tool: usize) {
        if self.selected_tool == tool {
            return;
        }
        self.prev_tool = self.selected_tool;
        self.selected_tool = tool;
    }

    pub(crate) fn select_prev_tool(&mut self) {
        self.selected_tool = self.prev_tool;
    }
}

impl egui_tiles::Behavior<DocumentTab> for DocumentBehavior {
    fn tab_title_for_pane(&mut self, pane: &DocumentTab) -> egui::WidgetText {
        let mut title = if let Some(file_name) = &pane.full_path {
            file_name.file_name().unwrap_or_default().to_str().unwrap_or_default().to_string()
        } else {
            fl!(crate::LANGUAGE_LOADER, "unsaved-title")
        };
        if pane.is_dirty() {
            title.push('*');
        }
        title.into()
    }

    fn is_tab_closable(&self, _tiles: &Tiles<DocumentTab>, _tile_id: TileId) -> bool {
        true
    }
    fn on_tab_close(&mut self, _tiles: &mut Tiles<DocumentTab>, tile_id: TileId) -> bool {
        self.request_close = Some(tile_id);
        false
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: egui_tiles::TileId, pane: &mut DocumentTab) -> egui_tiles::UiResponse {
        if pane.is_destroyed() {
            return egui_tiles::UiResponse::None;
        }

        let doc = &mut pane.doc.lock();
        self.message = doc.show_ui(ui, &mut self.tools.lock()[self.selected_tool], self.selected_tool, &self.document_options);

        let undo_stack_len = doc.undo_stack_len();
        if let Some(path) = &pane.full_path {
            if undo_stack_len != pane.auto_save_status {
                if pane.last_change_autosave_timer != undo_stack_len {
                    pane.instant = Instant::now();
                }
                pane.last_change_autosave_timer = undo_stack_len;

                if pane.instant.elapsed().as_secs() > 5 {
                    
                    pane.auto_save_status = undo_stack_len;
                    if let Ok(bytes) = doc.get_bytes(path) {
                        store_auto_save(path, &bytes);
                    }
                }
            }
        }

        egui_tiles::UiResponse::None
    }

    fn on_tab_button(&mut self, tiles: &Tiles<DocumentTab>, tile_id: TileId, button_response: eframe::egui::Response) -> Response {
        let response_opt = button_response.context_menu(|ui| {
            if ui.button(fl!(crate::LANGUAGE_LOADER, "tab-context-menu-close")).clicked() {
                self.request_close = Some(tile_id);
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button(fl!(crate::LANGUAGE_LOADER, "tab-context-menu-close_others")).clicked() {
                self.request_close_others = Some(tile_id);
                ui.close_kind(egui::UiKind::Menu);
            }
            if ui.button(fl!(crate::LANGUAGE_LOADER, "tab-context-menu-close_all")).clicked() {
                self.request_close_all = Some(tile_id);
                ui.close_kind(egui::UiKind::Menu);
            }
            ui.separator();
            if ui.button(fl!(crate::LANGUAGE_LOADER, "tab-context-menu-copy_path")).clicked() {
                if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(tile_id) {
                    if let Some(path) = &pane.full_path {
                        let text = path.to_string_lossy().to_string();
                        ui.ctx().copy_text(text);
                    }
                }
                ui.close_kind(egui::UiKind::Menu);
            }
        });
        if let Some(response_opt) = response_opt {
            response_opt.response
        } else {
            button_response
        }
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }

    fn top_bar_right_ui(&mut self, tiles: &Tiles<DocumentTab>, ui: &mut Ui, _tile_id: TileId, tabs: &Tabs, _scroll_offset: &mut f32) {
        if let Some(id) = tabs.active {
            if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(id) {
                let doc = &mut pane.doc.lock();
                if let Some(editor) = doc.get_ansi_editor() {
                    ui.add_space(4.0);
                    let mut buffer = Buffer::new((48, 1));
                    let font_page = editor.buffer_view.lock().get_caret().font_page();
                    if let Some(font) = editor.buffer_view.lock().get_buffer().font(font_page) {
                        buffer.set_font(1, font.clone());
                    }

                    let char_set = Settings::get_character_set();
                    if self.cur_char_set != char_set || self.dark_mode != ui.style().visuals.dark_mode {
                        let c = if ui.style().visuals.dark_mode {
                            ui.style().visuals.extreme_bg_color
                        } else {
                            (Rgba::from(ui.style().visuals.panel_fill) * Rgba::from_gray(0.8)).into()
                        };

                        let bg_color = buffer.palette.insert_color_rgb(c.r(), c.g(), c.b());

                        let c = ui.style().visuals.strong_text_color();
                        let fg_color = buffer.palette.insert_color_rgb(c.r(), c.g(), c.b());

                        let mut attr: TextAttribute = TextAttribute::default();
                        attr.set_background(bg_color);
                        attr.set_foreground(fg_color);
                        let s = format!("Set {:2} ", char_set + 1);
                        let mut i = 0;
                        for c in s.chars() {
                            buffer.layers[0].set_char((i, 0), AttributedChar::new(c, attr));
                            i += 1;
                        }
                        attr.set_foreground(15);
                        attr.set_background(4);

                        for j in i..buffer.width() {
                            buffer.layers[0].set_char((j, 0), AttributedChar::new(' ', attr));
                        }

                        for j in 0..10 {
                            if j == 9 {
                                i += 1;
                            }
                            let s = format!("{:-2}=", j + 1);
                            attr.set_foreground(0);
                            for c in s.chars() {
                                buffer.layers[0].set_char((i, 0), AttributedChar::new(c, attr));
                                i += 1;
                            }
                            attr.set_foreground(15);
                            attr.set_font_page(1);
                            buffer.layers[0].set_char((i, 0), AttributedChar::new(editor.get_char_set_key(j), attr));
                            attr.set_font_page(0);
                            i += 1;
                        }

                        self.char_set_img = Some(create_image(ui.ctx(), &buffer));
                    }

                    if let Some(handle) = &self.char_set_img {
                        let mut img = egui::Image::from_texture(handle);
                        img = img.sense(Sense::click());
                        let res = img.ui(ui);
                        if res.clicked_by(egui::PointerButton::Primary) {
                            Settings::set_character_set((char_set + 1) % DEFAULT_CHAR_SET_TABLE.len())
                        } else if res.clicked_by(egui::PointerButton::Secondary) {
                            Settings::set_character_set((char_set + DEFAULT_CHAR_SET_TABLE.len() - 1) % DEFAULT_CHAR_SET_TABLE.len())
                        }
                    }

                    if let Some(img) = &self.pos_img {
                        egui::Image::from_texture(img).ui(ui);
                    }
                }
            }
        }
    }
}

pub fn add_child(tree: &mut egui_tiles::Tree<DocumentTab>, full_path: Option<PathBuf>, doc: Box<dyn Document>) {
    let tile = DocumentTab {
        full_path,
        doc: Arc::new(Mutex::new(doc)),
        auto_save_status: 0,
        last_save: 0,
        instant: Instant::now(),
        last_change_autosave_timer: 0,
        destroyed: false,
    };
    let new_child = tree.tiles.insert_pane(tile);

    if let Some(root) = tree.root {
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) = tree.tiles.get_mut(root) {
            tabs.add_child(new_child);
            tabs.set_active(new_child);
        } else if let Some(egui_tiles::Tile::Pane(_)) = tree.tiles.get(root) {
            let new_id = tree.tiles.insert_tab_tile(vec![new_child, root]);
            tree.root = Some(new_id);
        } else {
            tree.root = Some(new_child);
        }
    } else {
        tree.root = Some(new_child);
    }
}
