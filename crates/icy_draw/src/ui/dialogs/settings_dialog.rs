use std::sync::Arc;

use eframe::{
    egui::{self, color_picker, Layout, Modifiers, RichText},
    epaint::{mutex::Mutex, Color32, Vec2},
};
use egui::{global_theme_preference_switch, Context};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, BitFont, Buffer, Color, Position, Size, TextAttribute};
use icy_engine_gui::{show_monitor_settings, show_terminal_area, BufferView, MarkerSettings, MonitorSettings};

use crate::{CharSetMapping, CharTableToolWindow, Commands, FontSelector, ModalDialog, SelectOutlineDialog, Settings, CHARACTER_SETS, KEYBINDINGS, SETTINGS};
pub struct SettingsDialog {
    settings_category: usize,
    select_outline_dialog: SelectOutlineDialog,
    is_dark_mode: Option<bool>,
    monitor_settings: MonitorSettings,
    marker_settings: MarkerSettings,
    key_filter: String,
    key_bindings: Vec<(String, eframe::egui::Key, Modifiers)>,

    font_cache: Vec<BitFont>,
    font_selector: Option<FontSelector>,
    cur_char_set: usize,
    char_sets: Vec<CharSetMapping>,
    views: Vec<Arc<Mutex<BufferView>>>,
    selected_view: usize,
    char_view: CharTableToolWindow,
}
const MONITOR_CAT: usize = 0;
const MARKER_CAT: usize = 1;
const OUTLINE_CAT: usize = 2;
const CHAR_SET_CAT: usize = 3;
const KEYBIND_CAT: usize = 4;

impl SettingsDialog {
    pub fn new(ctx: &Context, gl: &Arc<glow::Context>) -> Self {
        let mut views = Vec::new();

        for _ in 0..15 {
            let mut buffer = Buffer::new(Size::new(10, 1));
            buffer.is_terminal_buffer = false;
            let mut buffer_view = BufferView::from_buffer(gl, buffer);
            buffer_view.interactive = true;
            views.push(Arc::new(Mutex::new(buffer_view)));
        }
        let char_view = CharTableToolWindow::new(ctx, 32);

        let mut font_cache = if let Ok(font_dir) = Settings::get_font_diretory() {
            FontSelector::load_fonts(font_dir.as_path())
        } else {
            Vec::new()
        };

        for f in icy_engine::SAUCE_FONT_NAMES {
            font_cache.push(BitFont::from_sauce_name(f).unwrap());
        }
        for slot in 0..icy_engine::ANSI_FONTS {
            let ansi_font = BitFont::from_ansi_font_page(slot).unwrap();
            font_cache.push(ansi_font);
        }
        font_cache.dedup_by(|x, y| x.get_checksum() == y.get_checksum());
        let font = BitFont::default();
        font_cache.retain(|x| x.get_checksum() != font.get_checksum());
        font_cache.insert(0, font);

        Self {
            settings_category: MONITOR_CAT,
            select_outline_dialog: SelectOutlineDialog::default(),
            monitor_settings: Default::default(),
            marker_settings: Default::default(),
            key_filter: String::new(),
            key_bindings: Commands::default_keybindings(),
            char_sets: Default::default(),
            font_cache,
            cur_char_set: 0,
            selected_view: 0,
            views,
            char_view,
            font_selector: None,
            is_dark_mode: unsafe { SETTINGS.is_dark_mode },
        }
    }

    pub(crate) fn init(&mut self) {
        self.monitor_settings = unsafe { SETTINGS.monitor_settings.clone() };
        self.marker_settings = unsafe { SETTINGS.marker_settings.clone() };
        self.key_bindings = unsafe { KEYBINDINGS.key_bindings.clone() };
        self.char_sets = unsafe { CHARACTER_SETS.character_sets.clone() };
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        let mut dialog_open = true;
        let title = RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-heading"));
        if ctx.input(|i| i.key_down(egui::Key::Escape)) {
            open = false;
        }

        if let Some(selector) = &mut self.font_selector {
            if selector.show(ctx) {
                if selector.should_commit() {
                    let font = selector.selected_font().get_checksum();
                    let mut new_set = self.char_sets[0].clone();
                    new_set.font_checksum = font;
                    self.char_sets.push(new_set);
                }
                self.font_selector = None;
            }
            return open;
        }

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .fixed_size(Vec2::new(400., 300.))
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    global_theme_preference_switch(ui);
                    self.is_dark_mode = Some(ui.visuals().dark_mode);

                    let settings_category = self.settings_category;

                    if ui
                        .selectable_label(settings_category == MONITOR_CAT, fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"))
                        .clicked()
                    {
                        self.settings_category = MONITOR_CAT;
                    }

                    if ui
                        .selectable_label(settings_category == MARKER_CAT, fl!(crate::LANGUAGE_LOADER, "settings-markers-guides-category"))
                        .clicked()
                    {
                        self.settings_category = MARKER_CAT;
                    }

                    if ui
                        .selectable_label(settings_category == OUTLINE_CAT, fl!(crate::LANGUAGE_LOADER, "settings-font-outline-category"))
                        .clicked()
                    {
                        self.settings_category = OUTLINE_CAT;
                    }
                    if ui
                        .selectable_label(settings_category == CHAR_SET_CAT, fl!(crate::LANGUAGE_LOADER, "settings-char-set-category"))
                        .clicked()
                    {
                        self.settings_category = CHAR_SET_CAT;
                    }

                    if ui
                        .selectable_label(settings_category == KEYBIND_CAT, fl!(crate::LANGUAGE_LOADER, "settings-keybindings-category"))
                        .clicked()
                    {
                        self.settings_category = KEYBIND_CAT;
                    }
                });
                ui.separator();
                match self.settings_category {
                    MONITOR_CAT => unsafe {
                        if let Some(new_settings) = show_monitor_settings(ui, &SETTINGS.monitor_settings) {
                            SETTINGS.monitor_settings = new_settings;
                        }
                    },
                    MARKER_CAT => {
                        ui.add_space(8.0);
                        unsafe {
                            if let Some(new_settings) = show_marker_settings(ui, &SETTINGS.marker_settings) {
                                SETTINGS.marker_settings = new_settings;
                            }
                        }
                    }

                    CHAR_SET_CAT => {
                        ui.add_space(8.0);
                        self.show_charset_editor(ui);
                    }

                    OUTLINE_CAT => {
                        ui.add_space(8.0);
                        self.select_outline_dialog.show_outline_ui(ui, 4, Vec2::new(8.0, 8.0));
                    }

                    KEYBIND_CAT => {
                        let mut map = std::collections::HashMap::new();
                        for (s, key, modifier) in &self.key_bindings {
                            map.insert(s.clone(), (*key, *modifier));
                        }
                        crate::Commands::show_keybinds_settings(ui, &mut self.key_filter, &mut map);
                        self.key_bindings.clear();
                        for (s, (key, modifier)) in map {
                            self.key_bindings.push((s, key, modifier));
                        }
                    }
                    _ => {}
                }

                ui.separator();
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-ok")).clicked() {
                        unsafe {
                            if KEYBINDINGS.key_bindings != self.key_bindings {
                                KEYBINDINGS.key_bindings = self.key_bindings.clone();
                                if let Err(err) = KEYBINDINGS.save() {
                                    log::error!("Error saving keybindings: {}", err);
                                }
                            }
                            if CHARACTER_SETS.character_sets != self.char_sets {
                                CHARACTER_SETS.character_sets = self.char_sets.clone();
                                if let Err(err) = CHARACTER_SETS.save() {
                                    log::error!("Error saving character sets: {}", err);
                                }
                            }
                            SETTINGS.is_dark_mode = self.is_dark_mode;
                            if let Err(err) = Settings::save() {
                                log::error!("Error saving settings: {err}");
                            }
                        }
                        dialog_open = false;
                    }

                    if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                        unsafe {
                            SETTINGS.monitor_settings = self.monitor_settings.clone();
                            SETTINGS.marker_settings = self.marker_settings.clone();
                            if let Some(dark_mode) = SETTINGS.is_dark_mode {
                                ui.visuals_mut().dark_mode = dark_mode;
                            }
                        }
                        dialog_open = false;
                    }

                    if (self.settings_category == MONITOR_CAT
                        || self.settings_category == MARKER_CAT
                        || self.settings_category == CHAR_SET_CAT
                        || self.settings_category == KEYBIND_CAT)
                        && ui.button(fl!(crate::LANGUAGE_LOADER, "settings-reset_button")).clicked()
                    {
                        unsafe {
                            match self.settings_category {
                                MONITOR_CAT => SETTINGS.monitor_settings = Default::default(),
                                MARKER_CAT => SETTINGS.marker_settings = Default::default(),
                                CHAR_SET_CAT => self.char_sets = Default::default(),
                                KEYBIND_CAT => {
                                    self.key_bindings = Commands::default_keybindings();
                                }
                                _ => {}
                            }
                        }
                    }
                });
            });

        open && dialog_open
    }

    pub fn show_charset_editor(&mut self, ui: &mut egui::Ui) {
        ui.set_height(580.);
        let mut id = 0;
        ui.add_space(48.0);

        let mut cur_font = &self.font_cache[0];
        for font in &self.font_cache {
            if font.checksum == self.char_sets[self.cur_char_set].font_checksum {
                cur_font = font;
                break;
            }
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(fl!(crate::LANGUAGE_LOADER, "settings-char_set_list_label"));
                egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    ui.vertical(|ui| {
                        for (i, char_set) in self.char_sets.iter().enumerate() {
                            let label = if char_set.font_checksum == 0 {
                                "Default".to_string()
                            } else {
                                let mut result = "Unknown".to_string();
                                for font in &self.font_cache {
                                    if font.checksum == char_set.font_checksum {
                                        result = font.name.to_string();
                                        break;
                                    }
                                }
                                result
                            };
                            if ui.selectable_label(self.cur_char_set == i, label).clicked() {
                                self.cur_char_set = i;
                            }
                        }
                    });
                });
            });
            ui.separator();
            if ui.add(egui::Button::new(fl!(crate::LANGUAGE_LOADER, "add-font-dialog-select"))).clicked() {
                self.font_selector = Some(FontSelector::font_library());
            }

            if ui
                .add_enabled(
                    self.cur_char_set > 0,
                    egui::Button::new(fl!(crate::LANGUAGE_LOADER, "manage-font-remove_font_button")),
                )
                .clicked()
            {
                self.char_sets.remove(self.cur_char_set);
                self.cur_char_set = 0;
            }
            if ui
                .add_enabled(self.cur_char_set == 0, egui::Button::new(fl!(crate::LANGUAGE_LOADER, "settings-reset_button")))
                .clicked()
            {
                self.char_sets[0] = CharSetMapping::default();
                for view in self.views.iter() {
                    view.lock().get_edit_state_mut().set_is_buffer_dirty();
                }
            }
        });
        ui.separator();
        egui::Grid::new("paste_mode_grid").num_columns(6).spacing([8.0, 8.0]).show(ui, |ui| {
            for (i, view) in self.views.iter().enumerate() {
                let font_dims = view.lock().get_buffer_mut().get_font_dimensions();

                let opt = icy_engine_gui::TerminalOptions {
                    stick_to_bottom: false,
                    scale: Some(Vec2::new(2.0, 2.0)),
                    id: Some(egui::Id::new(200 + id)),
                    terminal_size: Some(Vec2::new(font_dims.width as f32 * 10. * 2.0, font_dims.height as f32 * 2.0)),
                    force_focus: self.selected_view == i,
                    ..Default::default()
                };

                for x in 0..10 {
                    let ch = self.char_sets[self.cur_char_set].table[id][x];
                    view.lock().get_buffer_mut().layers[0].set_char((x, 0), AttributedChar::new(ch, TextAttribute::default()));
                }
                if id % 3 == 0 {
                    ui.end_row();
                }
                id += 1;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if i == self.selected_view {
                        ui.strong(fl!(crate::LANGUAGE_LOADER, "settings-set-label", set = id));
                    } else {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "settings-set-label", set = id));
                    }
                });
                let (response, calc) = show_terminal_area(ui, view.clone(), opt);
                if response.has_focus() {
                    self.selected_view = i;
                }
                if response.clicked() {
                    if let Some(click_pos) = response.hover_pos() {
                        let pos = calc.calc_click_pos(click_pos);
                        let pos = Position::new(pos.x as i32, pos.y as i32);
                        view.lock().get_caret_mut().set_position(pos);
                    }
                }
            }
        });
        if self.char_view.get_font().get_checksum() != cur_font.checksum {
            self.char_view.set_font(ui.ctx(), cur_font.clone());
            for view in &self.views {
                view.lock().get_buffer_mut().set_font(0, cur_font.clone())
            }
        }
        if let Some(ch) = self.char_view.show_plain_char_table(ui) {
            let mut pos = self.views[self.selected_view].lock().get_caret().get_position();
            self.char_sets[self.cur_char_set].table[self.selected_view][pos.x as usize] = ch;
            pos.x = (pos.x + 1).min(9);
            self.views[self.selected_view].lock().get_caret_mut().set_position(pos);

            for x in 0..10 {
                let ch = self.char_sets[self.cur_char_set].table[self.selected_view][x];
                self.views[self.selected_view].lock().get_buffer_mut().layers[0].set_char((x, 0), AttributedChar::new(ch, TextAttribute::default()));
            }

            self.views[self.selected_view].lock().get_edit_state_mut().set_is_buffer_dirty();
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            let mut pos = self.views[self.selected_view].lock().get_caret().get_position();
            pos.x = (pos.x - 1).max(0);
            self.views[self.selected_view].lock().get_caret_mut().set_position(pos);
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            let mut pos = self.views[self.selected_view].lock().get_caret().get_position();
            pos.x = (pos.x + 1).min(9);
            self.views[self.selected_view].lock().get_caret_mut().set_position(pos);
        }
    }
}

pub fn show_marker_settings(ui: &mut egui::Ui, old_settings: &MarkerSettings) -> Option<MarkerSettings> {
    let mut result = None;

    let mut marker_settings = old_settings.clone();

    ui.add(egui::Slider::new(&mut marker_settings.reference_image_alpha, 0.1..=0.9).text(fl!(crate::LANGUAGE_LOADER, "settings-reference-alpha")));

    ui.horizontal(|ui| {
        ui.label(fl!(crate::LANGUAGE_LOADER, "settings-raster-label"));
        let (r, g, b) = marker_settings.raster_color.get_rgb();
        let mut color = Color32::from_rgb(r, g, b);

        color_picker::color_edit_button_srgba(ui, &mut color, color_picker::Alpha::Opaque);
        marker_settings.raster_color = Color::new(color.r(), color.g(), color.b());
        ui.add(egui::Slider::new(&mut marker_settings.raster_alpha, 0.1..=0.9).text(fl!(crate::LANGUAGE_LOADER, "settings-alpha")));
    });

    ui.horizontal(|ui| {
        ui.label(fl!(crate::LANGUAGE_LOADER, "settings-guide-label"));
        let (r, g, b) = marker_settings.guide_color.get_rgb();
        let mut color = Color32::from_rgb(r, g, b);

        color_picker::color_edit_button_srgba(ui, &mut color, color_picker::Alpha::Opaque);
        marker_settings.guide_color = Color::new(color.r(), color.g(), color.b());

        ui.add(egui::Slider::new(&mut marker_settings.guide_alpha, 0.1..=0.9).text(fl!(crate::LANGUAGE_LOADER, "settings-alpha")));
    });

    if marker_settings != *old_settings {
        result = Some(marker_settings);
    }

    result
}
