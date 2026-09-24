use super::widgets::{self, Icons};
use eframe::egui;
use icy_engine_gui::{MonitorSettings, ScalingMode, egui::screen::ScreenView};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub enum ExportFormat {
    Gif,
    Cast,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Cast => "cast",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Gif => "GIF Animation",
            Self::Cast => "Asciicast v2",
        }
    }
}

struct ExportJob {
    result: Receiver<Result<(), String>>,
    cancelled: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    count: usize,
}

pub struct AnimationEditor {
    pub source: String,
    baseline: String,
    pub path: Option<PathBuf>,
    animator: Arc<Mutex<Animator>>,
    compiling: Option<Arc<Mutex<Animator>>>,
    pending: bool,
    changed: Instant,
    frame: usize,
    shown_frame: Option<usize>,
    playing: bool,
    looping: bool,
    speed: f32,
    tick: Instant,
    preview: Option<ScreenView>,
    monitor: MonitorSettings,
    icons: Icons,
    export_request: Option<ExportFormat>,
    export_job: Option<ExportJob>,
    export_error: Option<String>,
    undo: Vec<String>,
    redo: Vec<String>,
}

impl AnimationEditor {
    pub fn new() -> Self {
        Self {
            source: String::new(),
            baseline: String::new(),
            path: None,
            animator: Arc::new(Mutex::new(Animator::default())),
            compiling: None,
            pending: false,
            changed: Instant::now(),
            frame: 0,
            shown_frame: None,
            playing: false,
            looping: true,
            speed: 1.0,
            tick: Instant::now(),
            preview: None,
            monitor: MonitorSettings {
                scaling_mode: ScalingMode::Auto,
                ..Default::default()
            },
            icons: Icons::default(),
            export_request: None,
            export_job: None,
            export_error: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut editor = Self::new();
        editor.baseline = source.clone();
        editor.source = source;
        editor.path = Some(path.to_path_buf());
        editor.compile();
        Ok(editor)
    }

    pub fn modified(&self) -> bool {
        self.source != self.baseline
    }

    pub fn replace_text(&mut self, offset: usize, length: usize, text: &str) -> Result<(), String> {
        let end = offset.saturating_add(length).min(self.source.len());
        if self.source.get(offset..end).is_none() {
            return Err("Invalid UTF-8 byte range".into());
        }
        self.undo.push(self.source.clone());
        self.redo.clear();
        self.source.replace_range(offset..end, text);
        self.pending = true;
        self.changed = Instant::now();
        Ok(())
    }

    pub fn undo_source(&mut self, redo: bool) {
        let (source, target) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(previous) = source.pop() {
            target.push(std::mem::replace(&mut self.source, previous));
            self.pending = true;
            self.changed = Instant::now();
        }
    }

    pub fn mcp_status(&self) -> icy_draw::mcp::types::AnimationStatus {
        let animator = self.animator.lock();
        icy_draw::mcp::types::AnimationStatus {
            text_length: self.source.len(),
            frame_count: animator.frames.len(),
            errors: if animator.error.is_empty() { vec![] } else { vec![animator.error.clone()] },
            is_playing: self.playing,
            current_frame: self.frame,
        }
    }

    pub fn mcp_screen(&self, frame: usize, format: icy_draw::mcp::types::ScreenCaptureFormat) -> Result<String, String> {
        let animator = self.animator.lock();
        let (screen, _, _) = animator.frames.get(frame.saturating_sub(1)).ok_or("Frame out of bounds")?;
        match format {
            icy_draw::mcp::types::ScreenCaptureFormat::Text => Ok((0..screen.height())
                .map(|row| {
                    (0..screen.width())
                        .map(|column| screen.buffer_type().convert_to_unicode(screen.char_at((column, row).into()).ch))
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")),
            icy_draw::mcp::types::ScreenCaptureFormat::Ansi => {
                let bytes = screen
                    .clone_box()
                    .to_bytes("ans", &icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal))
                    .map_err(|error| error.to_string())?;
                String::from_utf8(bytes).map_err(|error| error.to_string())
            }
        }
    }

    pub fn take_export_request(&mut self) -> Option<ExportFormat> {
        self.export_request.take()
    }

    #[cfg(test)]
    pub fn compile_for_test(&mut self) {
        self.compile();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.compiling.is_some() {
            self.poll();
            assert!(Instant::now() < deadline, "animation compilation timed out");
            std::thread::yield_now();
        }
        assert!(self.animator.lock().error.is_empty(), "{}", self.animator.lock().error);
    }

    #[cfg(test)]
    pub fn select_frame_for_test(&mut self, frame: usize) {
        self.frame = frame;
        self.poll();
    }

    #[cfg(test)]
    pub fn preview_rect_for_test(&self) -> egui::Rect {
        let info = self.preview.as_ref().unwrap().terminal.render_info.read();
        egui::Rect::from_min_size(egui::pos2(info.bounds_x, info.bounds_y), egui::vec2(info.viewport_width, info.viewport_height))
    }

    pub fn export(&mut self, path: PathBuf, format: ExportFormat, context: egui::Context) {
        if self.export_job.is_some() {
            return;
        }
        if self.path.as_deref().is_some_and(|source| icy_draw::files::same_file(source, &path))
            || !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension()))
        {
            self.export_error = Some(format!("Choose a different filename with a .{} extension.", format.extension()));
            return;
        }
        let animator = self.animator.clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicUsize::new(0));
        let (sender, result) = mpsc::channel();
        self.export_job = Some(ExportJob {
            result,
            cancelled: cancelled.clone(),
            progress: progress.clone(),
            count: animator.lock().frames.len(),
        });
        self.export_error = None;
        std::thread::spawn(move || {
            let result = export_frames(&animator, &path, format, &cancelled, &progress);
            let _ = sender.send(result);
            context.request_repaint();
        });
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        icy_draw::files::save_bytes(
            path,
            self.source.as_bytes(),
            self.path.as_deref().map(|source| (source, self.baseline.as_bytes())),
            overwrite,
        )?;
        self.path = Some(path.to_path_buf());
        self.baseline = self.source.clone();
        Ok(())
    }

    fn compile(&mut self) {
        if self.compiling.is_some() {
            self.pending = true;
            return;
        }
        let parent = self.path.as_ref().and_then(|path| path.parent()).map(Path::to_path_buf);
        self.compiling = Some(Animator::run(&parent, self.source.clone()));
        self.pending = false;
    }

    fn poll(&mut self) {
        if self.compiling.as_ref().is_some_and(|animator| !animator.lock().is_thread_running()) {
            self.animator = self.compiling.take().unwrap();
            self.frame = self.frame.min(self.animator.lock().frames.len().saturating_sub(1));
            self.shown_frame = None;
            self.preview = None;
        }
        if self.pending && self.compiling.is_none() && self.changed.elapsed() >= Duration::from_millis(700) {
            self.compile();
        }
        let animator = self.animator.lock();
        if self.playing {
            if let Some((_, _, delay)) = animator.frames.get(self.frame) {
                let duration = Duration::from_secs_f32((*delay as f32 / 1000.0 / self.speed).max(0.001));
                if self.tick.elapsed() >= duration {
                    if self.frame + 1 < animator.frames.len() {
                        self.frame += 1;
                    } else if self.looping {
                        self.frame = 0;
                    } else {
                        self.playing = false;
                    }
                    self.tick = Instant::now();
                }
            }
        }
        if self.shown_frame != Some(self.frame) {
            if let Some((screen, monitor, _)) = animator.frames.get(self.frame) {
                self.preview = Some(ScreenView::from_shared(Arc::new(Mutex::new(screen.clone_box()))));
                self.monitor.monitor_type = icy_engine_gui::MonitorType::from_index(i32::from(monitor.monitor_type) as usize);
                self.monitor.brightness = monitor.brightness;
                self.monitor.contrast = monitor.contrast;
                self.monitor.gamma = monitor.gamma;
                self.monitor.saturation = monitor.saturation;
                self.monitor.use_scanlines = monitor.use_scanlines;
                self.monitor.use_curvature = monitor.use_curvature;
                self.shown_frame = Some(self.frame);
            }
        }
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        if !blocked {
            let history = context.input_mut(|input| {
                if input.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)
                    || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                {
                    Some(true)
                } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                    Some(false)
                } else {
                    None
                }
            });
            if let Some(redo) = history {
                self.undo_source(redo);
            }
        }
        self.poll();
        if let Some(job) = &self.export_job {
            if let Ok(result) = job.result.try_recv() {
                self.export_error = result.err();
                self.export_job = None;
            }
        }
        if self.compiling.is_some() || self.pending || self.playing || self.export_job.is_some() {
            context.request_repaint_after(Duration::from_millis(16));
        }
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("animation-controls")
            .exact_height(44.0)
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(8, 0)))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                let count = self.animator.lock().frames.len();
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if self.icons.button(ui, "replay", "Compile", false).clicked() {
                        self.compile();
                    }
                    if self.compiling.is_some() {
                        ui.spinner();
                    }
                    widgets::divider(ui);
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if self.icons.button(ui, "navigate_prev", "Previous Frame", false).clicked() {
                        self.frame = self.frame.saturating_sub(1);
                        self.playing = false;
                    }
                    if self
                        .icons
                        .button(ui, if self.playing { "pause" } else { "play" }, "Play / Pause", self.playing)
                        .clicked()
                    {
                        self.playing = !self.playing;
                        self.tick = Instant::now();
                    }
                    if self.icons.button(ui, "navigate_next", "Next Frame", false).clicked() {
                        self.frame = (self.frame + 1).min(count.saturating_sub(1));
                        self.playing = false;
                    }
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::divider(ui);
                    widgets::toggle(ui, "Loop", &mut self.looping, "Restart at the first frame after the last one");
                    ui.add(egui::DragValue::new(&mut self.speed).range(0.1..=8.0).speed(0.1).suffix("×"))
                        .on_hover_text("Playback speed");
                    widgets::divider(ui);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(count > 0 && self.export_job.is_none(), |ui| {
                            ui.menu_button("Export", |ui| {
                                for format in [ExportFormat::Gif, ExportFormat::Cast] {
                                    if ui.button(format.name()).clicked() {
                                        self.export_request = Some(format);
                                        ui.close();
                                    }
                                }
                            });
                        });
                        if let Some(job) = &self.export_job {
                            if ui.button("Cancel").clicked() {
                                job.cancelled.store(true, Ordering::Relaxed);
                            }
                            ui.add(egui::ProgressBar::new(job.progress.load(Ordering::Relaxed) as f32 / job.count.max(1) as f32).desired_width(90.0));
                        }
                        if count > 0 {
                            ui.label(
                                egui::RichText::new(format!("{} / {count}", self.frame + 1))
                                    .size(12.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(40.0);
                                ui.add(egui::Slider::new(&mut self.frame, 0..=count - 1).show_value(false))
                                    .on_hover_text("Frame");
                            });
                        }
                    });
                });
            });
        let error = self.export_error.clone().unwrap_or_else(|| self.animator.lock().error.clone());
        if !error.is_empty() {
            egui::TopBottomPanel::bottom("animation-error")
                .max_height(130.0)
                .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(12, 8)))
                .show(context, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    });
                });
        }
        let source_frame = egui::Frame::new().fill(context.style().visuals.extreme_bg_color);
        if context.content_rect().width() >= 850.0 {
            egui::SidePanel::left("animation-source")
                .default_width(500.0)
                .width_range(260.0..=900.0)
                .resizable(true)
                .frame(source_frame)
                .show(context, |ui| self.code(ui, blocked));
        } else {
            egui::TopBottomPanel::top("animation-source-compact")
                .resizable(true)
                .default_height(220.0)
                .min_height(80.0)
                .frame(source_frame)
                .show(context, |ui| self.code(ui, blocked));
        }
        let well = if context.style().visuals.dark_mode {
            egui::Color32::from_gray(22)
        } else {
            egui::Color32::from_gray(212)
        };
        egui::CentralPanel::default().frame(egui::Frame::new().fill(well)).show(context, |ui| {
            if let Some(preview) = &mut self.preview {
                preview.show(ui, &self.monitor);
            }
        });
    }

    fn code(&mut self, ui: &mut egui::Ui, blocked: bool) {
        if blocked {
            ui.disable();
        }
        let previous = self.source.clone();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.source)
                        .id(egui::Id::new("animation-source-editor"))
                        .code_editor()
                        .frame(false)
                        .margin(egui::Margin::symmetric(12, 10))
                        .desired_width(f32::INFINITY)
                        .desired_rows(30),
                )
                .changed()
            {
                self.pending = true;
                self.changed = Instant::now();
                self.undo.push(previous);
                self.redo.clear();
            }
        });
    }
}

impl Drop for AnimationEditor {
    fn drop(&mut self) {
        if let Some(job) = &self.export_job {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

fn export_frames(
    animator: &Arc<Mutex<Animator>>,
    path: &Path,
    format: ExportFormat,
    cancelled: &AtomicBool,
    progress: &Arc<AtomicUsize>,
) -> Result<(), String> {
    let mut frames: Vec<_> = animator.lock().frames.iter().map(|(screen, _, delay)| (screen.clone_box(), *delay)).collect();
    let first = frames.first().ok_or("No frames to export")?;
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    match format {
        ExportFormat::Gif => {
            use icy_engine::gif_encoder::{GifEncoder, GifFrame, RepeatCount};
            let mut images = Vec::new();
            let mut dimensions = None;
            for (screen, delay) in &frames {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("Export cancelled".into());
                }
                let options = icy_engine::RenderOptions {
                    rect: icy_engine::Rectangle::from_coords(0, 0, screen.width(), screen.height()).into(),
                    blink_on: true,
                    ..Default::default()
                };
                let (size, pixels) = screen.render_to_rgba(&options);
                if size.width <= 0 || size.height <= 0 || size.width > u16::MAX as i32 || size.height > u16::MAX as i32 {
                    return Err("Invalid GIF frame dimensions".into());
                }
                if dimensions.is_some_and(|previous| previous != size) {
                    return Err("GIF export requires frames of equal size.".into());
                }
                dimensions = Some(size);
                images.push(GifFrame::new(pixels, *delay));
            }
            let size = dimensions.unwrap();
            let mut encoder = GifEncoder::new(size.width as u16, size.height as u16);
            encoder.set_repeat(RepeatCount::Infinite);
            let progress = progress.clone();
            encoder
                .encode_to_file_with_progress(
                    temporary.path(),
                    images,
                    move |current, _| {
                        progress.store(current, Ordering::Relaxed);
                    },
                    || cancelled.load(Ordering::Relaxed),
                )
                .map_err(|error| error.to_string())?;
        }
        ExportFormat::Cast => {
            let header = serde_json::json!({ "version": 2, "width": first.0.width(), "height": first.0.height() });
            writeln!(temporary, "{header}").map_err(|error| error.to_string())?;
            let mut timestamp = 0.0;
            for (index, (screen, delay)) in frames.iter_mut().enumerate() {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("Export cancelled".into());
                }
                let options = icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal);
                let bytes = screen.to_bytes("ans", &options).map_err(|error| error.to_string())?;
                let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
                let event = serde_json::json!([timestamp, "o", format!("\u{001b}[2J\u{001b}[H{text}")]);
                writeln!(temporary, "{event}").map_err(|error| error.to_string())?;
                timestamp += *delay as f64 / 1000.0;
                progress.store(index + 1, Ordering::Relaxed);
            }
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("Export cancelled".into());
    }
    temporary.as_file().sync_all().map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_frames_and_reports_lua_errors() {
        let mut editor = AnimationEditor::new();
        editor.source = "local screen = new_buffer(10, 3)\nnext_frame(screen)\nnext_frame(screen)".into();
        editor.compile();
        let deadline = Instant::now() + Duration::from_secs(5);
        while editor.compiling.is_some() {
            editor.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(editor.animator.lock().frames.len(), 2);
        assert!(editor.preview.is_some());
        editor.source = "this is not lua!".into();
        editor.compile();
        while editor.compiling.is_some() {
            editor.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!editor.animator.lock().error.is_empty());
        let directory = tempfile::tempdir().unwrap();
        editor.save(&directory.path().join("test.icyanim"), false).unwrap();
        assert!(!editor.modified());
    }

    #[test]
    fn exports_gif_and_cast_and_keeps_target_on_cancellation() {
        let animator = Arc::new(Mutex::new(Animator::default()));
        for _ in 0..2 {
            animator.lock().frames.push((
                Box::new(icy_engine::TextScreen::new((10, 3))),
                icy_engine_scripting::MonitorSettings::neutral(),
                100,
            ));
        }
        let directory = tempfile::tempdir().unwrap();
        let cancelled = AtomicBool::new(false);
        let progress = Arc::new(AtomicUsize::new(0));
        let gif = directory.path().join("test.gif");
        export_frames(&animator, &gif, ExportFormat::Gif, &cancelled, &progress).unwrap();
        assert_eq!(image::open(gif).unwrap().width(), 80);
        let cast = directory.path().join("test.cast");
        export_frames(&animator, &cast, ExportFormat::Cast, &cancelled, &progress).unwrap();
        let original = std::fs::read(&cast).unwrap();
        let events: Vec<serde_json::Value> = std::str::from_utf8(&original)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["version"], 2);
        assert_eq!(events[2][0], 0.1);
        cancelled.store(true, Ordering::Relaxed);
        assert!(export_frames(&animator, &cast, ExportFormat::Cast, &cancelled, &progress).is_err());
        assert_eq!(std::fs::read(cast).unwrap(), original);
    }
}
