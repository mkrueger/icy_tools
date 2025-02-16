#![allow(static_mut_refs)]
use eframe::{
    egui::{self, load::SizedTexture, Context, CursorIcon, Image, Margin, RichText, ScrollArea, TextureOptions},
    epaint::{Color32, ColorImage, Rect, Vec2},
    App, Frame,
};

use i18n_embed_fl::fl;
use icy_engine::{
    ansi::{
        sound::{MusicAction, FREQ},
        MusicOption,
    },
    parse_with_parser, rip, Buffer,
};
use icy_engine_gui::{animations::Animator, BufferView, MonitorSettings};
use igs::IGS;
use music::SoundThread;
use settings::{Settings, SETTINGS};
use settings_dialog::SettingsDialog;

use std::{
    env::current_dir,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{ItemFolder, ItemType};

use self::{
    file_view::{FileView, Message},
    options::{Options, ScrollSpeed},
};

mod file_view;
mod help_dialog;
mod igs;
mod music;
pub mod options;
pub mod rng;
mod sauce_dialog;
mod settings;
mod settings_dialog;

pub struct MainWindow<'a> {
    buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub file_view: FileView,
    pub in_scroll: bool,
    cur_scroll_pos: f32,
    pub last_scroll_pos: f32,
    drag_vel: f32,
    key_vel: f32,
    drag_started: bool,

    pub error_text: Option<String>,

    full_screen_mode: bool,
    hide_file_chooser: bool,

    loaded_buffer: bool,

    retained_image: Option<Image<'a>>,
    texture_handle: Option<ColorImage>,
    igs: Option<Arc<Mutex<IGS>>>,

    sauce_dialog: Option<sauce_dialog::SauceDialog>,
    help_dialog: Option<help_dialog::HelpDialog>,
    settings_dialog: Option<SettingsDialog>,

    toasts: egui_notify::Toasts,
    is_closed: bool,
    is_file_chooser: bool,
    pub is_canceled: bool,

    last_force_load: bool,
    pub opened_file: Option<usize>,
    pub store_options: bool,
    pub sound_thread: Arc<Mutex<SoundThread>>,

    // animations
    animation: Option<Arc<Mutex<Animator>>>,
}
pub const EXT_MUSIC_LIST: [&str; 2] = ["ams", "mus"];

pub const EXT_WHITE_LIST: [&str; 10] = ["seq", "diz", "nfo", "ice", "bbs", "ams", "mus", "txt", "doc", "md"];
pub const EXT_BLACK_LIST: [&str; 8] = ["zip", "rar", "gz", "tar", "7z", "pdf", "exe", "com"];
pub const EXT_IMAGE_LIST: [&str; 5] = ["png", "jpg", "jpeg", "gif", "bmp"];

impl<'a> App for MainWindow<'a> {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if !self.hide_file_chooser {
            egui::SidePanel::left("bottom_panel").exact_width(350.0).resizable(false).show(ctx, |ui| {
                if !(self.sauce_dialog.is_none() && self.help_dialog.is_none()) {
                    ui.disable();
                }
                let command = self.file_view.show_ui(ui, false);
                self.handle_command(ctx, command);
            });
        }
        let frame_no_margins = egui::containers::Frame::NONE
            .outer_margin(Margin::same(0))
            .inner_margin(Margin::same(0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default().frame(frame_no_margins).show(ctx, |ui| {
            if !(self.sauce_dialog.is_none() && self.help_dialog.is_none()) {
                ui.disable();
            }
            self.paint_main_area(ui)
        });

        self.in_scroll &= self.file_view.options.auto_scroll_enabled;
        if self.in_scroll {
            //   ctx.request_repaint_after(Duration::from_millis(10));
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(150));
        }

        if let Some(sauce_dialog) = &mut self.sauce_dialog {
            if let Some(message) = sauce_dialog.show(ctx) {
                match message {
                    sauce_dialog::Message::CloseDialog => {
                        self.sauce_dialog = None;
                    }
                }
            }
        }

        if let Some(help_dialog) = &mut self.help_dialog {
            if let Some(message) = help_dialog.show(ctx) {
                match message {
                    help_dialog::Message::CloseDialog => {
                        self.help_dialog = None;
                    }
                }
            }
        }

        if let Some(settings_dialog) = &mut self.settings_dialog {
            if !settings_dialog.show(ctx) {
                self.settings_dialog = None;
            }
        }

        self.toasts.show(ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::F9)) {
            self.hide_file_chooser = !self.hide_file_chooser;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F11) || i.key_pressed(egui::Key::Enter) && i.modifiers.alt) {
            self.full_screen_mode = !self.full_screen_mode;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.full_screen_mode));
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.alt) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.sauce_dialog.is_some() {
                self.sauce_dialog = None;
            } else if self.help_dialog.is_some() {
                self.help_dialog = None;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        if self.store_options {
            self.file_view.options.store_options();
        }
    }
}

const NOTE_TABLE: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

impl<'a> MainWindow<'a> {
    pub fn new(gl: &Arc<glow::Context>, mut initial_path: Option<PathBuf>, options: Options) -> Self {
        if let Ok(path) = Settings::get_settings_file() {
            if path.exists() {
                match Settings::load(&path) {
                    Ok(settings) => unsafe {
                        SETTINGS = settings;
                    },
                    Err(err) => {
                        log::error!("Error while loading settings: {err}");
                    }
                }
            }
        }
        let mut view = BufferView::new(gl);
        view.interactive = false;

        view.get_buffer_mut().is_terminal_buffer = false;
        view.get_caret_mut().set_is_visible(false);
        if let Some(path) = &initial_path {
            if path.is_relative() {
                if let Ok(cur) = current_dir() {
                    initial_path = Some(cur.join(path));
                }
            }
        }

        Self {
            buffer_view: Arc::new(eframe::epaint::mutex::Mutex::new(view)),
            file_view: FileView::new(initial_path, options),
            in_scroll: false,
            retained_image: None,
            texture_handle: None,
            igs: None,
            full_screen_mode: false,
            hide_file_chooser: false,
            error_text: None,
            loaded_buffer: false,
            sauce_dialog: None,
            help_dialog: None,
            settings_dialog: None,
            drag_started: false,
            cur_scroll_pos: 0.0,
            drag_vel: 0.0,
            key_vel: 0.0,
            last_scroll_pos: 1.0,
            toasts: egui_notify::Toasts::default(),
            opened_file: None,
            is_closed: false,
            is_canceled: false,
            animation: None,
            store_options: false,
            sound_thread: Arc::new(Mutex::new(SoundThread::new())),
            last_force_load: false,
            is_file_chooser: false,
        }
    }

    pub fn reset(&mut self) {
        self.in_scroll = false;
        self.retained_image = None;
        self.texture_handle = None;
        if let Some(igs) = self.igs.take() {
            igs.lock().unwrap().stop();
        }
        self.error_text = None;
        self.loaded_buffer = false;
        self.sauce_dialog = None;
        self.help_dialog = None;
        self.drag_started = false;
        self.cur_scroll_pos = 0.0;
        self.drag_vel = 0.0;
        self.key_vel = 0.0;
        self.last_scroll_pos = 1.0;
        self.opened_file = None;
        self.animation = None;
    }

    pub fn show_file_chooser(&mut self, ctx: &Context, monitor_settins: MonitorSettings) -> bool {
        self.is_closed = false;
        self.is_file_chooser = true;
        unsafe { SETTINGS.monitor_settings = monitor_settins };
        egui::SidePanel::left("bottom_panel").exact_width(412.0).resizable(false).show(ctx, |ui| {
            let command = self.file_view.show_ui(ui, true);
            self.handle_command(ctx, command);
        });

        let frame_no_margins = egui::containers::Frame::NONE
            .outer_margin(Margin::same(0))
            .inner_margin(Margin::same(0))
            .fill(Color32::BLACK);
        egui::CentralPanel::default().frame(frame_no_margins).show(ctx, |ui| self.paint_main_area(ui));
        self.file_view.options.show_settings = false;
        self.in_scroll &= self.file_view.options.auto_scroll_enabled;
        if self.in_scroll {
            //   ctx.request_repaint_after(Duration::from_millis(10));
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(150));
        }

        self.toasts.show(ctx);
        self.is_closed
    }

    fn paint_main_area(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = &self.error_text {
            ui.colored_label(ui.style().visuals.error_fg_color, err);
            return;
        }
        /*
        if let Some(image_loading_thread) = &self.image_loading_thread {
            if image_loading_thread.is_finished() {
                if let Some(img) = self.image_loading_thread.take() {
                    match img.join() {
                        Ok(img) => match img {
                            Ok(img) => {
                                self.retained_image = Some(img);
                            }
                            Err(err) => {
                                self.error_text = Some(err.to_string());
                            }
                        },
                        Err(err) => {
                            self.error_text = Some(format!("{err:?}"));
                        }
                    }
                } else {
                    self.error_text = Some(fl!(crate::LANGUAGE_LOADER, "error-never-happens").to_string());
                }
            } else {
                ui.centered_and_justified(|ui| ui.heading(fl!(crate::LANGUAGE_LOADER, "message-loading-image")));
            }
            return;
        } */

        if !self.buffer_view.lock().get_buffer().ansi_music.is_empty() {
            if let Ok(mut thread) = self.sound_thread.lock() {
                let _ = thread.update_state();

                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        if thread.is_playing() {
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "button-stop_music")).clicked() {
                                thread.clear();
                            }
                            ui.separator();
                            match &thread.cur_action {
                                Some(action) => match action {
                                    MusicAction::Pause(_) => {
                                        let duration = action.get_duration();
                                        ui.label(fl!(crate::LANGUAGE_LOADER, "label-music_pause", duration = duration));
                                    }
                                    MusicAction::PlayNote(freq, _len, _dotted) => {
                                        if let Some(item) = FREQ.iter().enumerate().find(|(_i, p)| *p == freq) {
                                            let note = item.0 % 12;
                                            let octave = item.0 / 12;
                                            let duration = action.get_duration();
                                            ui.label(fl!(
                                                crate::LANGUAGE_LOADER,
                                                "label-music_note",
                                                note = NOTE_TABLE[note],
                                                octave = octave,
                                                duration = duration
                                            ));
                                        }
                                    }
                                    _ => {}
                                },
                                None => {
                                    ui.label("No current action");
                                }
                            }
                        } else {
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "button-play_music")).clicked() {
                                for music in self.buffer_view.lock().get_buffer().ansi_music.iter().cloned() {
                                    let _ = thread.play_music(music);
                                }
                            }
                        }
                    });
                    ui.add_space(2.0);
                });
            }
        }
        if let Some(igs) = &self.igs {
            //ScrollArea::both().show(ui, |ui| {
            let color_image: ColorImage = igs.lock().unwrap().texture_handle.clone();
            let handle = ui.ctx().load_texture("my_texture", color_image, TextureOptions::NEAREST);
            let sized_texture: SizedTexture = (&handle).into();
            let w = ui.available_width();
            let scale = w / sized_texture.size.x;
            let img = Image::from_texture(sized_texture).fit_to_original_size(scale);
            let size = img.load_and_calc_size(ui, ui.available_size()).unwrap();
            let rect: Rect = egui::Rect::from_min_size(ui.min_rect().min, size);
            img.paint_at(ui, rect);
            //});
            return;
        }
        if let Some(img) = &self.retained_image {
            ScrollArea::both().show(ui, |ui| {
                let Some(size) = img.load_and_calc_size(ui, ui.available_size()) else {
                    return;
                };
                let rect: Rect = egui::Rect::from_min_size(ui.min_rect().min, size);
                img.paint_at(ui, rect);
            });
            return;
        }
        if let Some(texture_handle) = &self.texture_handle {
            ScrollArea::both().show(ui, |ui| {
                let color_image: ColorImage = texture_handle.clone();
                let handle = ui.ctx().load_texture("my_texture", color_image, TextureOptions::NEAREST);
                let sized_texture: SizedTexture = (&handle).into();
                let w = ui.available_width() - 16.0;
                let scale = w / sized_texture.size.x;
                let img = Image::from_texture(sized_texture).fit_to_original_size(scale);
                let size = img.load_and_calc_size(ui, ui.available_size()).unwrap();
                let rect: Rect = egui::Rect::from_min_size(ui.min_rect().min, size);
                img.paint_at(ui, rect);
            });
            return;
        }

        if let Some(anim) = &self.animation {
            let settings = anim.lock().unwrap().update_frame(self.buffer_view.clone());
            let (_, _) = self.show_buffer_view(ui, settings);
            return;
        }

        if self.loaded_buffer {
            let (response, calc) = self.show_buffer_view(ui, unsafe { SETTINGS.monitor_settings.clone() });

            // stop scrolling when reached the end.
            if self.in_scroll {
                if self.last_scroll_pos == calc.char_scroll_position.y {
                    self.in_scroll = false;
                }
                self.last_scroll_pos = calc.char_scroll_position.y;
            }
            self.cur_scroll_pos = calc.char_scroll_position.y;

            if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::Home) && i.modifiers.ctrl) {
                self.cur_scroll_pos = 0.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::End) && i.modifiers.ctrl) {
                self.cur_scroll_pos = f32::MAX;
                self.in_scroll = false;
            }

            if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
                self.key_vel = 500.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
                self.key_vel -= 250.0;
                self.in_scroll = false;
            }

            if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::PageUp) && i.modifiers.ctrl) {
                self.key_vel = 5000.0;
                self.in_scroll = false;
            }

            if ui.input(|i| i.key_pressed(egui::Key::PageDown) && i.modifiers.ctrl) {
                self.key_vel -= 2500.0;
                self.in_scroll = false;
            }
            let scroll_delta = ui.input_mut(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                self.cur_scroll_pos -= scroll_delta * 4.0;
                self.in_scroll = false;
            }

            if (self.key_vel - 0.1).abs() > 0.1 {
                let friction_coeff = 10.0;
                let dt = ui.input(|i| i.unstable_dt);
                let friction = friction_coeff * dt;
                self.key_vel -= friction * self.key_vel;
                self.cur_scroll_pos -= self.key_vel * dt;
                ui.ctx().request_repaint();
            }

            if response.drag_started_by(egui::PointerButton::Primary) {
                self.drag_started = false;
                if let Some(mouse_pos) = response.interact_pointer_pos() {
                    if !calc.vert_scrollbar_rect.contains(mouse_pos) && !calc.horiz_scrollbar_rect.contains(mouse_pos) {
                        self.drag_started = true;
                        ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);
                    }
                }
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                self.drag_started = false;
            }
            if response.dragged_by(egui::PointerButton::Primary) && self.drag_started {
                ui.input(|input| {
                    self.cur_scroll_pos -= input.pointer.delta().y;
                    self.drag_vel = input.pointer.velocity().y;
                    self.key_vel = 0.0;
                    self.in_scroll = false;
                });
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);
            } else {
                let friction_coeff = 10.0;
                let dt = ui.input(|i| i.unstable_dt);
                let friction = friction_coeff * dt;
                self.drag_vel -= friction * self.drag_vel;
                self.cur_scroll_pos -= self.drag_vel * dt;
                ui.ctx().request_repaint();
            }

            self.in_scroll &= !calc.set_scroll_position_set_by_user;
        } else {
            match self.file_view.selected_file {
                Some(file) => {
                    if self.file_view.files[file].is_folder() {
                        return;
                    }
                    ui.add_space(ui.available_height() / 3.0);
                    ui.vertical_centered(|ui| {
                        if let Some(idx) = self.file_view.selected_file {
                            ui.heading(fl!(
                                crate::LANGUAGE_LOADER,
                                "message-file-not-supported",
                                name = self.file_view.files[idx].get_label()
                            ));
                        }

                        ui.add_space(8.0);
                        if ui
                            .button(RichText::heading(fl!(crate::LANGUAGE_LOADER, "button-load-anyways").into()))
                            .clicked()
                        {
                            self.handle_command(ui.ctx(), Some(Message::Select(file, true)));
                        }
                    });
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.heading(fl!(crate::LANGUAGE_LOADER, "message-empty"));
                    });
                }
            }
        }
    }

    fn show_buffer_view(&mut self, ui: &mut egui::Ui, monitor_settings: MonitorSettings) -> (egui::Response, icy_engine_gui::TerminalCalc) {
        let w = if self.buffer_view.lock().get_buffer_mut().use_letter_spacing() {
            (ui.available_width() / 9.0).floor()
        } else {
            (ui.available_width() / 8.0).floor()
        };

        let scalex = (w / self.buffer_view.lock().get_width() as f32).min(2.0);
        let scaley = if self.buffer_view.lock().get_buffer_mut().use_aspect_ratio() {
            scalex * 1.35
        } else {
            scalex
        };

        let dt = ui.input(|i| i.unstable_dt);
        let sp = if self.in_scroll {
            (self.cur_scroll_pos + self.file_view.options.scroll_speed.get_speed() * dt).round()
        } else {
            self.cur_scroll_pos.round()
        };

        let mut opt = icy_engine_gui::TerminalOptions {
            stick_to_bottom: false,
            scale: Some(Vec2::new(scalex, scaley)),
            use_terminal_height: false,
            scroll_offset_y: Some(sp),
            monitor_settings,
            ..Default::default()
        };

        match self.buffer_view.lock().get_buffer().buffer_type {
            icy_engine::BufferType::Petscii => {
                opt.monitor_settings.border_color = icy_engine::Color::new(0x70, 0x7c, 0xE6);
            }

            icy_engine::BufferType::Unicode | icy_engine::BufferType::CP437 => {
                opt.monitor_settings.border_color = icy_engine::Color::new(64, 69, 74);
            }
            icy_engine::BufferType::Atascii => {
                opt.monitor_settings.border_color = icy_engine::Color::new(9, 0x51, 0x83);
            }
            icy_engine::BufferType::Viewdata => {
                opt.monitor_settings.border_color = icy_engine::Color::new(0, 0, 0);
            }
        }

        let (response, calc) = icy_engine_gui::show_terminal_area(ui, self.buffer_view.clone(), opt);
        (response, calc)
    }

    fn open_selected(&mut self, file: usize) -> bool {
        if file >= self.file_view.files.len() {
            return false;
        }

        if let Some(_) = self.file_view.files[file].get_subitems() {
            {
                let mut d = self.file_view.files.drain(file..);
                self.file_view.parents.push(d.next().unwrap());
            }
            self.file_view.refresh();
            self.file_view.selected_file = None;
            self.file_view.scroll_pos = None;
            self.reset_state();
            true
        } else {
            false
        }
    }

    fn view_selected(&mut self, ctx: &Context, file: usize, force_load: bool) {
        if file >= self.file_view.files.len() {
            if !self.is_file_chooser {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::DEFAULT_TITLE.clone()));
            }
            return;
        }
        if let Ok(thread) = self.sound_thread.lock() {
            thread.clear();
        }
        if let Some(igs) = self.igs.take() {
            igs.lock().unwrap().stop();
        }
        self.animation = None;
        self.last_scroll_pos = -1.0;
        let label = self.file_view.files[file].get_label();
        let path = self.file_view.files[file].get_file_path();
        let data = if self.file_view.files[file].item_type() != ItemType::Unknown || force_load {
            self.file_view.files[file].read_data().unwrap_or_default()
        } else {
            Vec::new()
        };

        match self.file_view.files[file].item_type() {
            ItemType::Folder => {
                if !self.is_file_chooser {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::DEFAULT_TITLE.clone()));
                }
                return;
            }
            ItemType::IcyAnimation => self.load_icy_animation(&path, &data),
            ItemType::Rip => self.load_rip(&data),
            ItemType::Picture => {
                let img = Image::from_bytes(label.clone(), data).show_loading_spinner(true);
                let img = img.texture_options(TextureOptions::NEAREST);
                self.retained_image = Some(img);
            }
            ItemType::IGS => self.load_igs(&path, &data),
            ItemType::Unknown => {
                if force_load {
                    self.load_ansi(&PathBuf::from("a.ans"), &data);
                }
            }
            ItemType::Ansi | ItemType::AnsiMusic => self.load_ansi(&path, &data),
        }
        if !self.is_file_chooser {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("iCY VIEW {} - {}", *crate::VERSION, label)));
        }
    }

    fn load_igs(&mut self, path: &Path, data: &[u8]) {
        match String::from_utf8(data.to_vec()) {
            Ok(data) => {
                let parent = path.parent().map(|path| path.to_path_buf());
                let anim = IGS::run(&parent, data);
                self.igs = Some(anim);
            }
            Err(err) => {
                log::error!("Error while parsing icyanim file: {err}");
                self.error_text = Some(format!("{}", err));
            }
        };
    }

    fn load_icy_animation(&mut self, path: &Path, data: &[u8]) {
        let anim: Result<Arc<Mutex<Animator>>, anyhow::Error> = match String::from_utf8(data.to_vec()) {
            Ok(data) => {
                let parent = path.parent().map(|path| path.to_path_buf());
                let anim: Arc<Mutex<Animator>> = Animator::run(&parent, data);
                anim.lock().unwrap().set_is_loop(true);
                anim.lock().unwrap().set_is_playing(true);
                Ok(anim)
            }
            Err(err) => {
                log::error!("Error while parsing icyanim file: {err}");
                Err(anyhow::anyhow!("Error while parsing icyanim file: {err}"))
            }
        };

        match anim {
            Ok(anim) => {
                anim.lock().unwrap().start_playback(self.buffer_view.clone());
                self.animation = Some(anim);
                return;
            }
            Err(err) => {
                log::error!("Error while loading icyanim file: {err}");
                self.error_text = Some(err.to_string())
            }
        }
    }

    fn load_rip(&mut self, data: &[u8]) {
        let buf: rip::Parser = {
            let mut rip_parser = rip::Parser::new(Box::default(), PathBuf::new());
            let mut result: Buffer = Buffer::new((80, 25));
            result.is_terminal_buffer = false;

            let (text, is_unicode) = icy_engine::convert_ansi_to_utf8(&data);
            if is_unicode {
                result.buffer_type = icy_engine::BufferType::Unicode;
            }

            match parse_with_parser(&mut result, &mut rip_parser, &text, true) {
                Ok(_) => rip_parser,
                Err(err) => {
                    log::error!("Error while parsing rip file: {err}");
                    rip_parser
                }
            }
        };
        let size = buf.bgi.window;
        let mut pixels = Vec::new();
        let pal = buf.bgi.get_palette().clone();
        for i in buf.bgi.screen {
            let (r, g, b) = pal.get_rgb(i as u32);
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255);
        }
        let color_image: ColorImage = ColorImage::from_rgba_premultiplied([size.width as usize, size.height as usize], &pixels);
        self.texture_handle = Some(color_image);
    }

    fn load_ansi(&mut self, path: &Path, data: &[u8]) {
        match Buffer::from_bytes(path, true, &data, Some(MusicOption::Both), Some(self.file_view.terminal_width)) {
            Ok(buf) => {
                if let Ok(mut thread) = self.sound_thread.lock() {
                    for music in buf.ansi_music.iter().cloned() {
                        let _ = thread.play_music(music);
                    }
                }
                self.buffer_view.lock().set_buffer(buf);
                self.loaded_buffer = true;
                self.in_scroll = true;
            }
            Err(err) => self.error_text = Some(err.to_string()),
        }
    }

    fn reset_state(&mut self) {
        self.retained_image = None;
        self.texture_handle = None;
        self.error_text = None;
        self.loaded_buffer = false;
        self.file_view.selected_file = None;
        self.cur_scroll_pos = 0.0;
    }

    pub fn handle_command(&mut self, ctx: &Context, command: Option<Message>) {
        if let Some(command) = command {
            match command {
                Message::Select(file, force_load) => {
                    if self.file_view.selected_file != Some(file) || force_load {
                        self.reset_state();
                        if file < self.file_view.files.len() {
                            self.file_view.selected_file = Some(file);
                            self.file_view.scroll_pos = Some(file);
                            self.last_force_load = force_load;
                            self.view_selected(ctx, file, force_load);
                        }
                    }
                }
                Message::Refresh => {
                    self.reset_state();
                    self.file_view.refresh();
                }
                Message::Open(file) => {
                    self.is_closed = !self.open(ctx, file);
                    self.is_canceled = false;
                }
                Message::Reopen => {
                    if let Some(file) = self.file_view.selected_file {
                        self.reset_state();
                        self.file_view.selected_file = Some(file);
                        self.view_selected(ctx, file, self.last_force_load);
                    }
                }
                Message::Cancel => {
                    self.is_closed = true;
                    self.is_canceled = true;
                }
                Message::ParentFolder => {
                    if self.file_view.parents.len() > 1 {
                        self.file_view.parents.pop();
                        self.reset_state();
                        self.file_view.refresh();
                        self.handle_command(ctx, Some(Message::Select(0, false)));
                    } else {
                        if let Some(parent) = self.file_view.parents.pop() {
                            if let Some(parent) = parent.get_file_path().parent() {
                                self.file_view.parents.push(Box::new(ItemFolder::new(parent.to_path_buf())));
                                self.reset_state();
                                self.file_view.refresh();
                                self.handle_command(ctx, Some(Message::Select(0, false)));
                            }
                        }
                    }
                }
                Message::ToggleAutoScroll => {
                    self.file_view.options.auto_scroll_enabled = !self.file_view.options.auto_scroll_enabled;
                    self.in_scroll = self.file_view.options.auto_scroll_enabled;

                    if self.file_view.options.auto_scroll_enabled {
                        self.toasts.info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-on"));
                        //.set_duration(Some(Duration::from_secs(3)));
                    } else {
                        self.toasts.info(fl!(crate::LANGUAGE_LOADER, "toast-auto-scroll-off"));
                        //.set_duration(Some(Duration::from_secs(3)));
                    }
                }
                Message::ShowSauce(file) => {
                    if file < self.file_view.files.len() {
                        if let Some(sauce) = self.file_view.files[file].get_sauce() {
                            self.sauce_dialog = Some(sauce_dialog::SauceDialog::new(sauce));
                        }
                    }
                }
                Message::ShowHelpDialog => {
                    self.help_dialog = Some(help_dialog::HelpDialog::new());
                }
                Message::ShowSettings => {
                    self.settings_dialog = Some(SettingsDialog::new());
                }
                Message::ChangeScrollSpeed => {
                    self.file_view.options.scroll_speed = self.file_view.options.scroll_speed.next();

                    match self.file_view.options.scroll_speed {
                        ScrollSpeed::Slow => {
                            self.toasts.info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-slow"));
                            //.set_duration(Some(Duration::from_secs(3)));
                        }
                        ScrollSpeed::Medium => {
                            self.toasts.info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-medium"));
                            //.set_duration(Some(Duration::from_secs(3)));
                        }
                        ScrollSpeed::Fast => {
                            self.toasts.info(fl!(crate::LANGUAGE_LOADER, "toast-scroll-fast"));
                            //.set_duration(Some(Duration::from_secs(3)));
                        }
                    }
                }
                Message::SetTerminalWidth => {
                    self.file_view.show_terminal_width = true;
                }
            }
        }
    }

    fn open(&mut self, ctx: &Context, file: usize) -> bool {
        if self.open_selected(file) && !self.file_view.files.is_empty() {
            self.file_view.selected_file = Some(0);
            self.file_view.scroll_pos = Some(0);
            self.view_selected(ctx, file, false);
            true
        } else {
            self.opened_file = Some(file);
            false
        }
    }

    pub fn get_terminal_width(&self) -> usize {
        self.file_view.terminal_width
    }
}
