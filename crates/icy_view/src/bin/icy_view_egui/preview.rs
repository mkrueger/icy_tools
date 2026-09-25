use eframe::egui;
use icy_engine::formats::FileFormat;
use icy_engine_gui::{egui::screen::ScreenView, CheckerboardColors, MonitorSettings, ScalingMode};
use icy_view::{
    tracker::{self, TrackerPlayer},
    view_thread::{ScrollMode, ViewCommand, ViewEvent, ViewThread},
    Options,
};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

/// Position of a file streamed through the parser (baud emulation playback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playback {
    pub position: usize,
    pub length: usize,
    pub paused: bool,
    pub cursor_px: i32,
}

impl Playback {
    pub fn finished(&self) -> bool {
        self.position >= self.length
    }

    pub fn playing(&self) -> bool {
        !self.paused && !self.finished()
    }
}

/// Load generation plus the module's info sheet, details and player.
type MusicLoad = (u64, Result<(icy_engine::TextBuffer, tracker::ModuleInfo, TrackerPlayer), String>);

pub struct Preview {
    pub screen: ScreenView,
    command: tokio::sync::mpsc::UnboundedSender<ViewCommand>,
    events: mpsc::Receiver<ViewEvent>,
    image_sender: mpsc::Sender<(u64, Result<image::RgbaImage, String>)>,
    image_receiver: mpsc::Receiver<(u64, Result<image::RgbaImage, String>)>,
    music_sender: mpsc::Sender<MusicLoad>,
    music_receiver: mpsc::Receiver<MusicLoad>,
    /// Tracker module playing in the background of its info sheet.
    pub music: Option<TrackerPlayer>,
    pub music_info: Option<tracker::ModuleInfo>,
    /// Start modules playing as soon as they are shown (off in shuffle mode).
    pub music_autoplay: bool,
    /// Play through the output device; tests turn this off.
    pub audio: bool,
    pub image: Option<egui::TextureHandle>,
    pub image_pixels: Option<image::RgbaImage>,
    image_tiles: Vec<(egui::Vec2, egui::TextureHandle)>,
    generation: u64,
    pub loading: bool,
    pub file: String,
    pub data: Arc<Vec<u8>>,
    pub sauce: Option<icy_sauce::SauceRecord>,
    pub content_size: usize,
    pub error: Option<String>,
    pub scroll_mode: ScrollMode,
    pub follow_cursor: bool,
    pub manual_scrolled: bool,
    pub baud: u32,
    pub playback: Option<Playback>,
    pub loaded_at: Instant,
    last_tick: Instant,
    selection_anchor: Option<icy_engine::Position>,
    accept_events: bool,
    loading_image: bool,
    loading_music: bool,
    auto: bool,
}

impl Preview {
    pub fn new(context: &egui::Context) -> anyhow::Result<Self> {
        let mut screen = FileFormat::XBin.from_bytes(include_bytes!("../../../data/welcome.xb"), None)?.screen;
        icy_engine_gui::version_helper::replace_version_marker(&mut screen.buffer, &icy_view::VERSION, None);
        screen.caret.visible = false;
        let screen = ScreenView::new(screen);
        let (command, mut events) = ViewThread::spawn(screen.terminal.screen.clone());
        let (sender, receiver) = mpsc::channel();
        let context = context.clone();
        std::thread::spawn(move || {
            while let Some(event) = events.blocking_recv() {
                if sender.send(event).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        let (image_sender, image_receiver) = mpsc::channel();
        let (music_sender, music_receiver) = mpsc::channel();
        Ok(Self {
            screen,
            command,
            events: receiver,
            image_sender,
            image_receiver,
            music_sender,
            music_receiver,
            music: None,
            music_info: None,
            music_autoplay: true,
            audio: true,
            image: None,
            image_pixels: None,
            image_tiles: Vec::new(),
            generation: 0,
            loading: false,
            file: String::new(),
            data: Arc::new(Vec::new()),
            sauce: None,
            content_size: 0,
            error: None,
            scroll_mode: ScrollMode::Off,
            follow_cursor: true,
            manual_scrolled: false,
            baud: 0,
            playback: None,
            loaded_at: Instant::now(),
            last_tick: Instant::now(),
            selection_anchor: None,
            accept_events: false,
            loading_image: false,
            loading_music: false,
            auto: false,
        })
    }

    pub fn set_baud(&mut self, rate: u32) {
        self.baud = rate;
        let _ = self.command.send(ViewCommand::SetBaudEmulation(if rate == 0 {
            icy_parser_core::BaudEmulation::Off
        } else {
            icy_parser_core::BaudEmulation::Rate(rate)
        }));
    }

    /// Change the rate; a finished stream replays so the new speed can be watched.
    pub fn change_baud(&mut self, rate: u32) {
        self.set_baud(rate);
        if rate != 0 && self.playback.is_some_and(|playback| playback.finished()) {
            self.replay();
        }
    }

    pub fn toggle_pause(&mut self) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        if playback.finished() {
            self.replay();
            return;
        }
        playback.paused = !playback.paused;
        let _ = self.command.send(ViewCommand::SetPaused(playback.paused));
    }

    pub fn replay(&mut self) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        playback.position = 0;
        playback.paused = false;
        self.loading = true;
        self.follow_cursor = true;
        self.loaded_at = Instant::now();
        self.screen.scroll_to = Some(egui::Vec2::ZERO);
        let _ = self.command.send(ViewCommand::Seek(0));
        let _ = self.command.send(ViewCommand::SetPaused(false));
    }

    /// Jump to a byte position and pause there.
    pub fn seek(&mut self, position: usize) {
        let Some(playback) = &mut self.playback else {
            return;
        };
        playback.position = position.min(playback.length);
        playback.paused = true;
        // Seeking to the end completes the load in the view thread.
        self.loading = true;
        let _ = self.command.send(ViewCommand::SetPaused(true));
        let _ = self.command.send(ViewCommand::Seek(position));
    }

    fn paused(&self) -> bool {
        self.playback.is_some_and(|playback| playback.paused)
    }

    pub fn stop(&mut self) {
        self.generation += 1;
        self.playback = None;
        self.music = None;
        self.music_info = None;
        self.loading_music = false;
        self.loading = false;
        self.accept_events = false;
        self.scroll_mode = ScrollMode::Off;
        self.follow_cursor = true;
        self.manual_scrolled = false;
        let had_file = !self.file.is_empty();
        self.file.clear();
        self.image = None;
        self.image_pixels = None;
        self.image_tiles.clear();
        self.screen.scroll_to = Some(egui::Vec2::ZERO);
        if had_file {
            *self.screen.terminal.screen.lock() = Box::new(icy_engine::TextScreen::new(icy_engine::Size::new(80, 25)));
            self.screen.terminal.update_viewport_size();
        }
        let _ = self.command.send(ViewCommand::Stop);
    }

    /// Replaces the loaded screen with a buffer rendered on the UI thread (font samples).
    pub fn show_buffer(&mut self, buffer: icy_engine::TextBuffer, reset_scroll: bool) {
        use icy_engine::EditableScreen;
        let mut screen = icy_engine::TextScreen::from_buffer(buffer);
        screen.terminal_state_mut().is_terminal_buffer = false;
        *self.screen.terminal.screen.lock() = Box::new(screen);
        self.screen.terminal.update_viewport_size();
        self.selection_anchor = None;
        if reset_scroll {
            self.screen.scroll_to = Some(egui::Vec2::ZERO);
        }
    }

    pub fn load(&mut self, path: String, data: Vec<u8>, auto: bool, context: &egui::Context) {
        self.stop();
        self.file = path;
        self.sauce = icy_sauce::SauceRecord::from_bytes(&data).ok().flatten();
        self.content_size = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All).len();
        self.data = Arc::new(data);
        self.image = None;
        self.image_pixels = None;
        self.image_tiles.clear();
        self.playback = None;
        self.error = None;
        self.screen.scroll_to = Some(egui::Vec2::ZERO);
        self.loading = true;
        self.loaded_at = Instant::now();
        self.auto = auto;
        let format = FileFormat::from_path(std::path::Path::new(&self.file));
        self.loading_image = matches!(format, Some(FileFormat::Image(_)));
        self.loading_music = tracker::is_tracker_file(std::path::Path::new(&self.file));
        if self.loading_music {
            let (path, data, generation) = (PathBuf::from(&self.file), self.data.clone(), self.generation);
            let (sender, context) = (self.music_sender.clone(), context.clone());
            let (audio, paused) = (self.audio, !self.music_autoplay);
            std::thread::spawn(move || {
                let result = tracker::load_module(&path, &data).map(|module| {
                    let info = tracker::ModuleInfo::new(&module, &data);
                    let buffer = tracker::render_info(&info);
                    let player = if audio {
                        TrackerPlayer::start(module, paused)
                    } else {
                        TrackerPlayer::silent(&module, paused)
                    };
                    (buffer, info, player)
                });
                let _ = sender.send((generation, result.map_err(|error| error.to_string())));
                context.request_repaint();
            });
        } else if self.loading_image {
            let data = self.data.clone();
            let generation = self.generation;
            let sender = self.image_sender.clone();
            let context = context.clone();
            std::thread::spawn(move || {
                let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);
                let result = match format {
                    Some(FileFormat::Image(format)) => format.decode_rgba(data),
                    _ => Err("not an image".to_string()),
                };
                let _ = sender.send((generation, result));
                context.request_repaint();
            });
        } else {
            self.load_text();
        }
    }

    fn load_text(&mut self) {
        let _ = self.command.send(ViewCommand::LoadDataTagged(
            self.generation,
            PathBuf::from(&self.file),
            self.data.as_ref().clone(),
            self.auto,
        ));
    }

    pub fn toggle_music(&mut self) {
        if let Some(music) = &self.music {
            if music.finished() {
                music.seek(0.0);
                music.set_paused(false);
            } else {
                music.set_paused(!music.paused());
            }
        }
    }

    pub fn replay_music(&mut self) {
        if let Some(music) = &self.music {
            music.seek(0.0);
            music.set_paused(false);
        }
    }

    pub fn poll(&mut self, context: &egui::Context) {
        if self.music.as_ref().is_some_and(|music| music.playing()) {
            context.request_repaint_after(Duration::from_millis(250));
        }
        while let Ok(event) = self.events.try_recv() {
            if self.loading_image || self.loading_music {
                continue;
            }
            let event = match event {
                ViewEvent::ForRequest(request, event) if request == self.generation => *event,
                _ => continue,
            };
            if let ViewEvent::LoadingStarted(path) = &event {
                self.accept_events = path == &PathBuf::from(&self.file);
            }
            if !self.accept_events {
                continue;
            }
            match event {
                ViewEvent::LoadingCompleted => {
                    self.loading = false;
                    self.loaded_at = Instant::now();
                }
                ViewEvent::LoadFailed(error) => {
                    self.loading = false;
                    self.error = Some(error);
                }
                ViewEvent::SetScrollMode(mode) => self.scroll_mode = mode,
                ViewEvent::SauceInfo(sauce, _) => self.sauce = sauce,
                ViewEvent::Progress(position, length, cursor_px) => {
                    let paused = self.paused();
                    self.playback = Some(Playback {
                        position,
                        length,
                        paused,
                        cursor_px,
                    });
                    if position < length {
                        self.loading = true;
                    }
                }
                ViewEvent::LoadingStarted(_) => {}
                ViewEvent::ForRequest(_, _) => {}
            }
        }
        while let Ok((generation, result)) = self.music_receiver.try_recv() {
            if generation != self.generation || !self.loading_music {
                continue;
            }
            self.loading_music = false;
            match result {
                Ok((buffer, info, player)) => {
                    self.show_buffer(buffer, true);
                    self.music = Some(player);
                    self.music_info = Some(info);
                    self.loading = false;
                    self.loaded_at = Instant::now();
                }
                // Not a module after all (".mod" is also used for kernel and Fortran modules).
                Err(_) => self.load_text(),
            }
        }
        while let Ok((generation, result)) = self.image_receiver.try_recv() {
            if generation != self.generation {
                continue;
            }
            self.loading = false;
            match result {
                Ok(pixels) => {
                    let limit = context.input(|input| input.max_texture_side).clamp(1, 2048) as u32;
                    for top in (0..pixels.height()).step_by(limit as usize) {
                        for left in (0..pixels.width()).step_by(limit as usize) {
                            let tile =
                                image::imageops::crop_imm(&pixels, left, top, limit.min(pixels.width() - left), limit.min(pixels.height() - top)).to_image();
                            let texture = context.load_texture(
                                "preview-image",
                                egui::ColorImage::from_rgba_unmultiplied([tile.width() as usize, tile.height() as usize], tile.as_raw()),
                                egui::TextureOptions::LINEAR,
                            );
                            self.image_tiles.push((egui::vec2(left as f32, top as f32), texture));
                        }
                    }
                    self.image = self.image_tiles.first().map(|(_, texture)| texture.clone());
                    self.image_pixels = Some(pixels);
                    self.scroll_mode = ScrollMode::AutoScroll;
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, options: &Options) {
        self.manual_scrolled = false;
        let delta = self.last_tick.elapsed().as_secs_f32().min(0.1);
        self.last_tick = Instant::now();
        if self.follow_cursor && self.scroll_mode == ScrollMode::ClampToBottom && self.loading && !self.paused() {
            if let Some(playback) = self.playback {
                let bottom = playback.cursor_px as f32 * self.screen.zoom;
                self.screen.scroll_to = Some(egui::vec2(self.screen.offset.x, (bottom - ui.available_height()).max(0.0)));
            }
        } else if options.auto_scroll_enabled && !self.loading && !self.file.is_empty() && self.loaded_at.elapsed() > Duration::from_secs(1) {
            self.screen.scroll_to = Some(self.screen.offset + egui::vec2(0.0, options.scroll_speed.get_speed() * delta));
            if self.screen.offset.y < self.screen.max_offset.y {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
        }
        let previous_offset = self.screen.offset;
        let requested_offset = self.screen.scroll_to;
        let response = if let Some(pixels) = &self.image_pixels {
            let available = ui.available_size().max(egui::Vec2::splat(1.0));
            let size = egui::vec2(pixels.width() as f32, pixels.height() as f32);
            self.screen.zoom =
                viewer_scaling(&options.monitor_settings).compute_zoom(size.x, size.y, available.x, available.y, options.monitor_settings.use_integer_scaling);
            let scaled = size * self.screen.zoom;
            self.screen.max_offset = (scaled - available).max(egui::Vec2::ZERO);
            let mut area = egui::ScrollArea::both().id_salt("image").auto_shrink([false, false]);
            if let Some(offset) = self.screen.scroll_to.take() {
                area = area.scroll_offset(offset.max(egui::Vec2::ZERO).min(self.screen.max_offset));
            }
            let result = area.show(ui, |ui| {
                let (rect, response) = ui.allocate_exact_size(scaled, egui::Sense::click_and_drag());
                for (offset, texture) in &self.image_tiles {
                    let bounds = egui::Rect::from_min_size(rect.min + *offset * self.screen.zoom, texture.size_vec2() * self.screen.zoom);
                    if ui.is_rect_visible(bounds) {
                        egui::Image::new(texture).paint_at(ui, bounds);
                    }
                }
                response
            });
            self.screen.offset = result.state.offset;
            result.inner
        } else {
            let mut settings = options.monitor_settings.clone();
            if self.file.is_empty() {
                settings = MonitorSettings::neutral();
                settings.scaling_mode = ScalingMode::Auto;
            } else {
                settings.scaling_mode = viewer_scaling(&options.monitor_settings);
            }
            if self.playback.is_some() {
                settings.checkerboard_colors = CheckerboardColors::new(icy_engine::Color::new(0, 0, 0), icy_engine::Color::new(0, 0, 0), 8.0);
            }
            let response = self.screen.show(ui, &settings);
            self.select(&response);
            if let Some(url) = super::link_at(&self.screen.terminal, response.hover_pos()) {
                response.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }
            response
        };
        let expected_offset = requested_offset.unwrap_or(previous_offset).max(egui::Vec2::ZERO).min(self.screen.max_offset);
        self.manual_scrolled = ui.input(|input| {
            input.pointer.primary_down()
                && input.pointer.hover_pos().is_some_and(|pos| ui.max_rect().contains(pos))
                && (self.screen.offset - expected_offset).length() > 1.0
        });
        if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
            self.screen.scroll_to = Some(self.screen.offset - ui.input(|input| input.pointer.delta()));
            self.manual_scrolled = true;
        }
        if self.loading && !self.paused() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }

    fn select(&mut self, response: &egui::Response) {
        let info = self.screen.terminal.render_info.read();
        let cell = |pointer: egui::Pos2| {
            info.screen_to_cell(pointer.x, pointer.y).map(|(column, row)| {
                icy_engine::Position::new(
                    column + (self.screen.terminal.scroll_x() / info.font_width.max(1.0)) as i32,
                    row + (self.screen.terminal.scroll_y() / info.font_height.max(1.0)) as i32,
                )
            })
        };
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.selection_anchor = response.ctx.input(|input| input.pointer.press_origin()).and_then(cell);
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(anchor), Some(lead)) = (self.selection_anchor, response.interact_pointer_pos().and_then(cell)) {
                let mut selection = icy_engine::Selection::new(anchor);
                selection.lead = lead;
                if response.ctx.input(|input| input.modifiers.alt) {
                    selection.shape = icy_engine::Shape::Rectangle;
                }
                let _ = self.screen.terminal.screen.lock().set_selection(selection);
            }
        }
        if let Some(position) = response.interact_pointer_pos().and_then(cell) {
            let mut screen = self.screen.terminal.screen.lock();
            if response.triple_clicked() {
                let mut selection = icy_engine::Selection::new((0, position.y));
                selection.lead = (screen.width() - 1, position.y).into();
                let _ = screen.set_selection(selection);
            } else if response.double_clicked() {
                let mut left = position.x;
                let mut right = position.x;
                let is_word = |column| {
                    !screen
                        .buffer_type()
                        .convert_to_unicode(screen.char_at((column, position.y).into()).ch)
                        .is_whitespace()
                };
                while left > 0 && is_word(left - 1) {
                    left -= 1;
                }
                while right + 1 < screen.width() && is_word(right + 1) {
                    right += 1;
                }
                let mut selection = icy_engine::Selection::new((left, position.y));
                selection.lead = (right, position.y).into();
                let _ = screen.set_selection(selection);
            } else if response.clicked() {
                let _ = screen.clear_selection();
            }
        }
    }

    pub fn copy(&self, context: &egui::Context) {
        if let Some(pixels) = &self.image_pixels {
            context.copy_image(egui::ColorImage::from_rgba_unmultiplied(
                [pixels.width() as usize, pixels.height() as usize],
                pixels.as_raw(),
            ));
        } else if let Some(text) = self.screen.terminal.screen.lock().copy_text() {
            context.copy_text(text);
        }
    }

    pub fn select_all(&mut self) {
        let mut screen = self.screen.terminal.screen.lock();
        let mut selection = icy_engine::Selection::new((0, 0));
        selection.lead = (screen.width() - 1, screen.height() - 1).into();
        let _ = screen.set_selection(selection);
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.command.send(ViewCommand::Shutdown);
    }
}

/// The viewer fills the width and scrolls vertically; it never shrinks art to fit the height.
pub fn viewer_scaling(settings: &MonitorSettings) -> ScalingMode {
    match settings.scaling_mode {
        ScalingMode::Manual(zoom) => ScalingMode::Manual(zoom),
        _ => ScalingMode::FitWidth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tall_text_art_fills_width_and_scrolls_vertically() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        let mut data = Vec::new();
        for row in 0..200 {
            data.extend_from_slice(format!("LINE {row:03}\r\n").as_bytes());
        }
        preview.load("tall.ans".into(), data, false, &context);
        crate::tests::wait_preview(&mut preview, &context);
        let options = Options {
            auto_scroll_enabled: false,
            ..Default::default()
        };
        let frame = |preview: &mut Preview| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 400.0))),
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| preview.show(ui, &options));
                },
            );
        };
        for _ in 0..2 {
            frame(&mut preview);
        }
        assert!(preview.screen.max_offset.y > 0.0, "tall art is not scrollable");
        assert_eq!(preview.screen.max_offset.x, 0.0, "fitting the width must not scroll horizontally");
        preview.screen.scroll_to = Some(egui::vec2(0.0, f32::MAX));
        for _ in 0..2 {
            frame(&mut preview);
        }
        assert!(preview.screen.offset.y > 0.0, "scrolling to the bottom had no effect");
    }

    fn wait(preview: &mut Preview, context: &egui::Context, what: &str, done: impl Fn(&Preview) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(preview) {
            preview.poll(context);
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn row_start(preview: &Preview, row: i32) -> char {
        let screen = preview.screen.terminal.screen.lock();
        screen.buffer_type().convert_to_unicode(screen.char_at((0, row).into()).ch)
    }

    #[test]
    fn playback_pauses_seeks_and_replays_streamed_files() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        preview.set_baud(300);
        let mut data = Vec::new();
        for row in 0..20 {
            data.extend_from_slice(format!("ROW {row:02}\r\n").as_bytes());
        }
        let length = data.len();
        preview.load("stream.ans".into(), data, false, &context);
        wait(&mut preview, &context, "progress", |preview| preview.playback.is_some());
        assert_eq!(preview.playback.unwrap().length, length);
        assert!(preview.loading);

        preview.seek(length / 2);
        wait(&mut preview, &context, "forward seek", |preview| row_start(preview, 9) == 'R');
        assert_eq!(row_start(&preview, 10), ' ', "seeking must stop at the requested byte");
        std::thread::sleep(Duration::from_millis(200));
        preview.poll(&context);
        let playback = preview.playback.unwrap();
        assert!(playback.paused && playback.position == length / 2, "paused playback moved: {playback:?}");
        assert!(preview.loading);

        preview.seek(length);
        wait(&mut preview, &context, "seek to the end", |preview| !preview.loading);
        assert_eq!(row_start(&preview, 19), 'R');
        assert!(preview.playback.unwrap().finished());

        preview.seek(3);
        wait(&mut preview, &context, "backward seek", |preview| {
            preview.loading && row_start(preview, 1) == ' '
        });
        assert_eq!(row_start(&preview, 0), 'R');

        preview.change_baud(0);
        preview.toggle_pause();
        wait(&mut preview, &context, "resume", |preview| !preview.loading);
        assert_eq!(row_start(&preview, 19), 'R');

        preview.change_baud(300);
        assert!(preview.loading, "changing the rate of a finished file replays it");
        wait(&mut preview, &context, "replay restart", |preview| row_start(preview, 1) == ' ');
        assert!(preview.playback.unwrap().playing());
    }

    #[test]
    fn streaming_keeps_full_height_and_follows_the_cursor() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        preview.set_baud(300);
        let data = (0..80).map(|row| format!("ROW {row:02}\r\n")).collect::<String>().into_bytes();
        let length = data.len();
        preview.load("long.ans".into(), data, false, &context);
        wait(&mut preview, &context, "initial progress", |preview| preview.playback.is_some());
        let (height, window_height) = {
            let screen = preview.screen.terminal.screen.lock();
            (screen.height(), screen.terminal_state().height())
        };
        assert!(height >= 80, "full document height must be available before typing: {height}");
        assert_eq!(window_height, 25, "document height must not stretch the terminal viewport");
        let options = Options {
            auto_scroll_enabled: false,
            ..Default::default()
        };
        let frame = |preview: &mut Preview| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 240.0))),
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| preview.show(ui, &options));
                },
            );
        };
        frame(&mut preview);
        frame(&mut preview);
        let max_offset = preview.screen.max_offset.y;
        let zoom = preview.screen.zoom;
        assert!(max_offset > 0.0);
        assert_eq!(preview.screen.offset.y, 0.0, "start at the top, not at the blank end");
        preview.seek(length / 2);
        wait(&mut preview, &context, "cursor progress", |preview| {
            preview
                .playback
                .is_some_and(|playback| playback.position == length / 2 && playback.cursor_px >= 40 * 16)
        });
        preview.toggle_pause();
        frame(&mut preview);
        frame(&mut preview);
        assert!(preview.screen.offset.y > 0.0, "the viewport should follow the typing cursor");
        assert!(preview.screen.offset.y < max_offset, "the viewport should not jump to the document end");
        assert!((preview.screen.zoom - zoom).abs() < 0.01, "playback must not change the art's aspect ratio");
        assert_eq!(preview.screen.terminal.screen.lock().terminal_state().height(), window_height);
    }

    #[test]
    fn slow_baud_reveals_the_first_character_before_the_rest() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        preview.set_baud(300);
        preview.load("letters.ans".into(), b"ABCDEFGHIJ".to_vec(), false, &context);
        wait(&mut preview, &context, "first character", |preview| {
            preview.playback.is_some_and(|playback| playback.position >= 1)
        });
        assert_eq!(preview.playback.unwrap().position, 1);
        let screen = preview.screen.terminal.screen.lock();
        assert_eq!(screen.char_at((0, 0).into()).ch, b'A' as char);
        assert_ne!(screen.char_at((1, 0).into()).ch, b'B' as char);
    }

    #[test]
    fn switching_files_clears_previous_art_before_new_data_arrives() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        preview.set_baud(300);
        preview.load("first.ans".into(), b"OLD ART ".repeat(40), false, &context);
        wait(&mut preview, &context, "first file", |preview| row_start(preview, 0) == 'O');
        assert!(preview.loading, "the first stream must still be active");
        assert_eq!(row_start(&preview, 0), 'O');
        preview.stop();
        assert!(preview.file.is_empty());
        assert_ne!(row_start(&preview, 0), 'O');
        preview.load("second.ans".into(), b"NEW ART".to_vec(), false, &context);
        crate::tests::wait_preview(&mut preview, &context);
        assert_eq!(row_start(&preview, 0), 'N');
        assert_eq!(preview.file, "second.ans");
    }

    #[test]
    fn repeated_same_path_loads_ignore_old_completion() {
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        let (sender, receiver) = mpsc::channel();
        preview.events = receiver;
        preview.file = "same.ans".into();
        preview.loading = true;
        preview.generation = 2;
        for event in [ViewEvent::LoadingStarted("same.ans".into()), ViewEvent::LoadingCompleted] {
            sender.send(ViewEvent::ForRequest(1, Box::new(event))).unwrap();
        }
        preview.poll(&context);
        assert!(preview.loading);
        for event in [ViewEvent::LoadingStarted("same.ans".into()), ViewEvent::LoadingCompleted] {
            sender.send(ViewEvent::ForRequest(2, Box::new(event))).unwrap();
        }
        preview.poll(&context);
        assert!(!preview.loading);
    }
}
