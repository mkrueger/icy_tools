use eframe::egui;
use icy_engine::formats::{FileFormat, ImageFormat};
use icy_engine_gui::{egui::screen::ScreenView, MonitorSettings, ScalingMode};
use icy_view::{
    view_thread::{ScrollMode, ViewCommand, ViewEvent, ViewThread},
    Options,
};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

pub struct Preview {
    pub screen: ScreenView,
    command: tokio::sync::mpsc::UnboundedSender<ViewCommand>,
    events: mpsc::Receiver<ViewEvent>,
    image_sender: mpsc::Sender<(u64, Result<image::RgbaImage, String>)>,
    image_receiver: mpsc::Receiver<(u64, Result<image::RgbaImage, String>)>,
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
    pub baud: u32,
    pub loaded_at: Instant,
    last_tick: Instant,
    selection_anchor: Option<icy_engine::Position>,
    accept_events: bool,
    loading_image: bool,
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
        Ok(Self {
            screen,
            command,
            events: receiver,
            image_sender,
            image_receiver,
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
            baud: 0,
            loaded_at: Instant::now(),
            last_tick: Instant::now(),
            selection_anchor: None,
            accept_events: false,
            loading_image: false,
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

    pub fn stop(&mut self) {
        self.generation += 1;
        self.loading = false;
        self.accept_events = false;
        self.scroll_mode = ScrollMode::Off;
        let _ = self.command.send(ViewCommand::Stop);
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
        self.error = None;
        self.screen.scroll_to = Some(egui::Vec2::ZERO);
        self.loading = true;
        self.loaded_at = Instant::now();
        let format = FileFormat::from_path(std::path::Path::new(&self.file));
        self.loading_image = matches!(format, Some(FileFormat::Image(_)));
        if self.loading_image {
            let data = self.data.clone();
            let generation = self.generation;
            let sender = self.image_sender.clone();
            let context = context.clone();
            std::thread::spawn(move || {
                let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);
                let result = if format == Some(FileFormat::Image(ImageFormat::Sixel)) {
                    icy_sixel::SixelImage::decode(data).map_err(|error| error.to_string()).and_then(|image| {
                        image::RgbaImage::from_raw(image.width as u32, image.height as u32, image.pixels).ok_or_else(|| "Invalid Sixel dimensions".into())
                    })
                } else {
                    image::load_from_memory(data).map(|image| image.into_rgba8()).map_err(|error| error.to_string())
                };
                let _ = sender.send((generation, result));
                context.request_repaint();
            });
        } else {
            let _ = self.command.send(ViewCommand::LoadDataTagged(
                self.generation,
                PathBuf::from(&self.file),
                self.data.as_ref().clone(),
                auto,
            ));
        }
    }

    pub fn poll(&mut self, context: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            if self.loading_image {
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
                ViewEvent::LoadingStarted(_) => {}
                ViewEvent::ForRequest(_, _) => {}
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
        let delta = self.last_tick.elapsed().as_secs_f32().min(0.1);
        self.last_tick = Instant::now();
        if self.scroll_mode == ScrollMode::ClampToBottom && self.loading {
            self.screen.scroll_to = Some(egui::vec2(self.screen.offset.x, f32::MAX));
        } else if options.auto_scroll_enabled && !self.loading && !self.file.is_empty() && self.loaded_at.elapsed() > Duration::from_secs(1) {
            self.screen.scroll_to = Some(self.screen.offset + egui::vec2(0.0, options.scroll_speed.get_speed() * delta));
            if self.screen.offset.y < self.screen.max_offset.y {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
        }
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
        if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
            self.screen.scroll_to = Some(self.screen.offset - ui.input(|input| input.pointer.delta()));
        }
        if self.loading {
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
