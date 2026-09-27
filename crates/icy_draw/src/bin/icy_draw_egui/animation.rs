//! Lua animation editor: highlighted source with a log on the left, a player-style preview with
//! transport controls on the right, and an export dialog for GIF, AV1 and Asciicast.

pub use super::animation_export::ExportFormat;
use super::{
    animation_export::{export_frames, ExportProgress},
    widgets::Icons,
};
use eframe::egui::{self, Color32, Stroke, StrokeKind};
use icy_draw::fl;
use icy_engine_gui::{
    egui::{
        appearance::{self, labels, DialogButton, DialogSize, PRIMARY},
        dialog::DANGER,
        screen::ScreenView,
    },
    MonitorSettings, ScalingMode,
};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;
use std::sync::{
    atomic::Ordering,
    mpsc::{self, Receiver},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
const TRANSPORT_BUTTON: f32 = 30.0;
const PLAY_BUTTON: f32 = 40.0;
const SCRUBBER_HEIGHT: f32 = 22.0;
const CONTROL_BAR_HEIGHT: f32 = 54.0;
const STATUS_HEIGHT: f32 = 24.0;
const PLAY: Color32 = Color32::from_rgb(46, 160, 67);

struct ExportJob {
    result: Receiver<Result<(), String>>,
    progress: Arc<ExportProgress>,
}

/// State of the export dialog; the path is kept as typed so it can be edited freely.
struct ExportDialog {
    format: ExportFormat,
    path: String,
}

/// Work the editor needs from the application window, which owns the file pickers and confirmations.
pub enum Request {
    /// Pick the export target; answer with [`AnimationEditor::set_export_path`].
    Browse(ExportFormat),
    /// Export after confirming an overwrite; continue with [`AnimationEditor::export`].
    Export(PathBuf, ExportFormat),
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
    request: Option<Request>,
    export_dialog: Option<ExportDialog>,
    export_job: Option<ExportJob>,
    export_error: Option<String>,
    log_visible: bool,
    /// Caret line and column, both starting at 0.
    cursor: (usize, usize),
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
            request: None,
            export_dialog: None,
            export_job: None,
            export_error: None,
            log_visible: false,
            cursor: (0, 0),
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

    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    pub fn open_export_dialog(&mut self) {
        if self.export_dialog.is_none() {
            let path = self.path.as_ref().map(|path| path.with_extension(ExportFormat::Gif.extension()));
            self.export_dialog = Some(ExportDialog {
                format: ExportFormat::Gif,
                path: path.map(|path| path.display().to_string()).unwrap_or_default(),
            });
            self.export_error = None;
        }
    }

    /// Picked export target; its extension also selects the format.
    pub fn set_export_path(&mut self, mut path: PathBuf) {
        if let Some(dialog) = &mut self.export_dialog {
            match ExportFormat::ALL
                .into_iter()
                .find(|format| path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(format.extension())))
            {
                Some(format) => dialog.format = format,
                None => {
                    path.set_extension(dialog.format.extension());
                }
            }
            dialog.path = path.display().to_string();
            self.export_error = None;
        }
    }

    #[cfg(test)]
    pub fn compile_for_test(&mut self) {
        if self.compiling.is_none() {
            self.compile();
        }
        self.pending = false;
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
    pub fn export_dialog_open(&self) -> bool {
        self.export_dialog.is_some()
    }

    #[cfg(test)]
    pub fn show_log_for_test(&mut self, visible: bool) {
        self.log_visible = visible;
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
            self.export_error = Some(fl!("animation-export-extension", extension = format.extension()));
            return;
        }
        let animator = self.animator.clone();
        let progress = Arc::new(ExportProgress::default());
        progress.total.store(animator.lock().frames.len(), Ordering::Relaxed);
        let (sender, result) = mpsc::channel();
        self.export_job = Some(ExportJob {
            result,
            progress: progress.clone(),
        });
        self.export_error = None;
        std::thread::spawn(move || {
            let result = export_frames(&animator, &path, format, &progress);
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

    /// Playback position and total duration in milliseconds, from the frame delays.
    fn time_info(&self) -> (u64, u64) {
        let animator = self.animator.lock();
        let delays = animator.frames.iter().map(|(_, _, delay)| u64::from(*delay));
        let current = delays.clone().take(self.frame).sum();
        (current, delays.sum())
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) {
        let locked = blocked || self.export_dialog.is_some();
        if !locked {
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
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
                self.compile();
            }
        }
        self.poll();
        if let Some(job) = &self.export_job {
            if let Ok(result) = job.result.try_recv() {
                if result.is_ok() {
                    self.export_dialog = None;
                }
                self.export_error = result.err();
                self.export_job = None;
            }
        }
        if self.compiling.is_some() || self.pending || self.playing || self.export_job.is_some() {
            context.request_repaint_after(Duration::from_millis(16));
        }
        self.status_bar(context, locked);
        let source_frame = egui::Frame::new().fill(context.style().visuals.extreme_bg_color);
        let width = context.content_rect().width();
        if width >= 850.0 {
            egui::SidePanel::left("animation-source")
                .default_width((width * 0.45).clamp(420.0, 700.0))
                .width_range(260.0..=900.0)
                .resizable(true)
                .frame(source_frame)
                .show(context, |ui| self.source_pane(ui, locked));
        } else {
            egui::TopBottomPanel::top("animation-source-compact")
                .resizable(true)
                .default_height(220.0)
                .min_height(80.0)
                .frame(source_frame)
                .show(context, |ui| self.source_pane(ui, locked));
        }
        let panel_fill = context.style().visuals.panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(panel_fill).inner_margin(8))
            .show(context, |ui| {
                if locked {
                    ui.disable();
                }
                self.player(ui);
            });
        self.export_dialog(context, blocked);
    }

    /// Caret position, the latest log line (toggling the log) and the modified state.
    fn status_bar(&mut self, context: &egui::Context, locked: bool) {
        let visuals = context.style().visuals.clone();
        egui::TopBottomPanel::bottom("animation-status")
            .exact_height(STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(visuals.panel_fill).inner_margin(egui::Margin::symmetric(10, 0)))
            .show(context, |ui| {
                if locked {
                    ui.disable();
                }
                let rect = ui.max_rect();
                let third = rect.width() / 3.0;
                let column =
                    |index: f32, width: f32| egui::Rect::from_min_size(egui::pos2(rect.left() + third * index, rect.top()), egui::vec2(width, rect.height()));
                let small = |text: String| egui::RichText::new(text).size(12.0);
                let (line, col): (usize, usize) = (self.cursor.0 + 1, self.cursor.1 + 1);
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(column(0.0, third))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| ui.label(small(fl!("animation-cursor", line = line, column = col))),
                );
                let (message, error) = {
                    let animator = self.animator.lock();
                    if animator.error.is_empty() {
                        let message = animator
                            .log
                            .iter()
                            .rfind(|entry| entry.frame <= self.frame)
                            .map(|entry| format!("[{}] {}", entry.frame, entry.text));
                        (message.unwrap_or_else(|| fl!("animation-log")), false)
                    } else {
                        (animator.error.lines().next().unwrap_or_default().to_owned(), true)
                    }
                };
                let color = if error { visuals.error_fg_color } else { visuals.text_color() };
                let galley = ui.painter().layout_no_wrap(message, egui::FontId::proportional(12.0), color);
                let width = (14.0 + 4.0 + galley.size().x).min(third * 2.0);
                let toggle = egui::Rect::from_center_size(rect.center(), egui::vec2(width + 8.0, rect.height() - 4.0));
                let response = ui
                    .interact(toggle, egui::Id::new("animation-log-toggle"), egui::Sense::click())
                    .on_hover_text(fl!("animation-toggle-log"))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.hovered() {
                    ui.painter().rect_filled(toggle, 4, visuals.widgets.hovered.weak_bg_fill);
                }
                let angle = if self.log_visible { std::f32::consts::FRAC_PI_2 } else { 0.0 };
                let icon = egui::Rect::from_center_size(egui::pos2(toggle.left() + 4.0 + 7.0, rect.center().y), egui::Vec2::splat(14.0));
                self.icons
                    .image(ui, "navigate_next", 14.0)
                    .tint(color)
                    .rotate(angle, egui::Vec2::splat(0.5))
                    .paint_at(ui, icon);
                let text = egui::pos2(icon.right() + 4.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().with_clip_rect(toggle).galley(text, galley, color);
                if response.clicked() && !locked {
                    self.log_visible = !self.log_visible;
                }
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(column(2.0, third))
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        if self.modified() {
                            ui.label(small(fl!("animation-modified")).color(visuals.weak_text_color()));
                        }
                        if self.compiling.is_some() || self.pending {
                            ui.label(small(fl!("animation-compiling")).color(visuals.weak_text_color()));
                            ui.add(egui::Spinner::new().size(12.0));
                        }
                    },
                );
            });
    }

    fn source_pane(&mut self, ui: &mut egui::Ui, locked: bool) {
        if locked {
            ui.disable();
        }
        if self.log_visible {
            let visuals = ui.visuals().clone();
            egui::TopBottomPanel::bottom("animation-log")
                .resizable(true)
                .default_height(140.0)
                .height_range(60.0..=400.0)
                .frame(
                    egui::Frame::new()
                        .fill(visuals.panel_fill)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .stroke(visuals.widgets.noninteractive.bg_stroke),
                )
                .show_inside(ui, |ui| self.log(ui));
        }
        self.code(ui);
    }

    /// Script error, or the log entries written up to the shown frame.
    fn log(&self, ui: &mut egui::Ui) {
        let (error, entries) = {
            let animator = self.animator.lock();
            let entries: Vec<_> = animator
                .log
                .iter()
                .filter(|entry| entry.frame <= self.frame)
                .map(|entry| (entry.frame, entry.text.clone()))
                .collect();
            (animator.error.clone(), entries)
        };
        egui::ScrollArea::vertical()
            .id_salt("animation-log-entries")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if !error.is_empty() {
                    ui.label(egui::RichText::new(error).monospace().color(ui.visuals().error_fg_color));
                } else if entries.is_empty() {
                    ui.weak(fl!("animation-no-log"));
                } else {
                    for (frame, text) in entries {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("[{frame}]")).monospace().weak());
                            ui.label(egui::RichText::new(text).monospace());
                        });
                    }
                }
            });
    }

    fn code(&mut self, ui: &mut egui::Ui) {
        let previous = self.source.clone();
        let colors = SyntaxColors::new(ui.visuals());
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let mut job = egui::text::LayoutJob::default();
            for (span, color) in lua_spans(text.as_str(), &colors) {
                job.append(span, 0.0, egui::TextFormat::simple(font.clone(), color));
            }
            // Code is not wrapped; the scroll area scrolls horizontally instead.
            let _ = wrap_width;
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let output = egui::TextEdit::multiline(&mut self.source)
                .id(egui::Id::new("animation-source-editor"))
                .code_editor()
                .frame(false)
                .margin(egui::Margin::symmetric(12, 10))
                .desired_width(f32::INFINITY)
                .desired_rows(30)
                .layouter(&mut layouter)
                .show(ui);
            if let Some(range) = output.cursor_range {
                let index = range.primary.index;
                let before: String = self.source.chars().take(index).collect();
                let line = before.matches('\n').count();
                let col = before.rsplit('\n').next().map_or(0, |line| line.chars().count());
                self.cursor = (line, col);
            }
            if output.response.changed() {
                self.pending = true;
                self.changed = Instant::now();
                self.undo.push(previous);
                self.redo.clear();
            }
        });
    }

    /// Preview well with frame and time overlays, the frame scrubber and the transport bar.
    fn player(&mut self, ui: &mut egui::Ui) {
        let count = self.animator.lock().frames.len();
        let full = ui.available_rect_before_wrap();
        let controls = SCRUBBER_HEIGHT + CONTROL_BAR_HEIGHT + 12.0;
        let preview = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), (full.bottom() - controls).max(full.top() + 40.0)));
        let scrubber = egui::Rect::from_min_size(egui::pos2(full.left(), preview.bottom() + 6.0), egui::vec2(full.width(), SCRUBBER_HEIGHT));
        let bar = egui::Rect::from_min_size(egui::pos2(full.left(), scrubber.bottom() + 6.0), egui::vec2(full.width(), CONTROL_BAR_HEIGHT));
        self.preview_well(ui, preview, count);
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(scrubber.shrink2(egui::vec2(4.0, 0.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.add_enabled_ui(count > 1, |ui| {
                    let mut position = self.frame as f32;
                    let max = count.saturating_sub(1).max(1) as f32;
                    if appearance::slider(ui, &mut position, 0.0..=max, ui.available_width())
                        .on_hover_text(fl!("animation-frame"))
                        .changed()
                    {
                        self.frame = (position.round() as usize).min(count.saturating_sub(1));
                        self.tick = Instant::now();
                    }
                });
            },
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| self.transport(ui, count));
        ui.advance_cursor_after_rect(full);
    }

    fn preview_well(&mut self, ui: &mut egui::Ui, rect: egui::Rect, count: usize) {
        let visuals = ui.visuals().clone();
        let well = if visuals.dark_mode { Color32::from_gray(22) } else { Color32::from_gray(212) };
        ui.painter().rect(rect, 6, well, visuals.widgets.noninteractive.bg_stroke, StrokeKind::Inside);
        let inner = rect.shrink(4.0);
        let error = self.animator.lock().error.clone();
        let message = if !error.is_empty() {
            Some(egui::RichText::new(error).color(visuals.error_fg_color))
        } else if count == 0 && (self.compiling.is_some() || self.pending) {
            Some(egui::RichText::new(fl!("animation-compiling")).weak())
        } else if count == 0 {
            Some(egui::RichText::new(fl!("animation-no-frames")).weak())
        } else {
            None
        };
        if let Some(message) = message {
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner.shrink(16.0)), |ui| {
                ui.centered_and_justified(|ui| ui.add(egui::Label::new(message.size(14.0)).wrap()));
            });
            return;
        }
        if let Some(preview) = &mut self.preview {
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| preview.show(ui, &self.monitor));
        }
        let (current, total) = self.time_info();
        let painter = ui.painter().with_clip_rect(inner);
        let current_frame: usize = self.frame + 1;
        let frame = fl!("animation-frame-display", current = current_frame, total = count);
        overlay_label(&painter, inner.left_top() + egui::vec2(8.0, 8.0), egui::Align::LEFT, frame);
        let time = format!("{} / {}", format_time(current), format_time(total));
        overlay_label(&painter, inner.right_top() + egui::vec2(-8.0, 8.0), egui::Align::RIGHT, time);
    }

    fn transport(&mut self, ui: &mut egui::Ui, count: usize) {
        let visuals = ui.visuals().clone();
        let rect = ui.max_rect();
        ui.painter()
            .rect(rect, 8, visuals.faint_bg_color, visuals.widgets.noninteractive.bg_stroke, StrokeKind::Inside);
        let ready = count > 0;
        let last = count.saturating_sub(1);
        // first, previous, play, next, last, restart, loop; with wider gaps around play and before restart.
        let group = 6.0 * TRANSPORT_BUTTON + PLAY_BUTTON + 4.0 * 7.0 + 4.0 * 2.0 + 12.0;
        let speed_width = 76.0;
        let left = (rect.center().x - group / 2.0)
            .min(rect.right() - 12.0 - speed_width - 12.0 - group)
            .max(rect.left() + 12.0);
        let buttons = egui::Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(rect.right(), rect.bottom()));
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(buttons)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let stop = |editor: &mut Self, frame: usize| {
                    editor.frame = frame;
                    editor.playing = false;
                };
                if transport_button(&mut self.icons, ui, "first_page", &fl!("animation-first-frame"), ready && self.frame > 0, false).clicked() {
                    stop(self, 0);
                }
                if transport_button(
                    &mut self.icons,
                    ui,
                    "skip_previous",
                    &fl!("animation-previous-frame"),
                    ready && self.frame > 0,
                    false,
                )
                .clicked()
                {
                    stop(self, self.frame.saturating_sub(1));
                }
                ui.add_space(4.0);
                if play_button(&mut self.icons, ui, self.playing, ready).clicked() {
                    if !self.playing && self.frame >= last && !self.looping {
                        self.frame = 0;
                    }
                    self.playing = !self.playing;
                    self.tick = Instant::now();
                }
                ui.add_space(4.0);
                if transport_button(
                    &mut self.icons,
                    ui,
                    "skip_next",
                    &fl!("animation-next-frame"),
                    ready && self.frame < last,
                    false,
                )
                .clicked()
                {
                    stop(self, (self.frame + 1).min(last));
                }
                if transport_button(
                    &mut self.icons,
                    ui,
                    "last_page",
                    &fl!("animation-last-frame"),
                    ready && self.frame < last,
                    false,
                )
                .clicked()
                {
                    stop(self, last);
                }
                ui.add_space(12.0);
                if transport_button(&mut self.icons, ui, "replay", &fl!("animation-restart"), ready, false).clicked() {
                    self.frame = 0;
                    self.playing = true;
                    self.tick = Instant::now();
                }
                let tooltip = format!("{}\n{}", fl!("animation-loop"), fl!("animation-loop-tooltip"));
                if transport_button(&mut self.icons, ui, "repeat", &tooltip, ready, self.looping).clicked() {
                    self.looping = !self.looping;
                }
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(12.0, 0.0)))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                egui::ComboBox::from_id_salt("animation-speed")
                    .width(speed_width - 12.0)
                    .selected_text(speed_label(self.speed))
                    .show_ui(ui, |ui| {
                        for speed in SPEEDS {
                            ui.selectable_value(&mut self.speed, speed, speed_label(speed));
                        }
                    })
                    .response
                    .on_hover_text(fl!("animation-speed"));
            },
        );
    }

    /// Format, target and progress of an export; the export itself runs in the background.
    fn export_dialog(&mut self, context: &egui::Context, blocked: bool) {
        #[derive(Clone, Copy)]
        enum Action {
            Close,
            Stop,
            Export,
        }
        if self.export_dialog.is_none() {
            return;
        }
        let (frames, size) = {
            let animator = self.animator.lock();
            let size = animator.frames.first().map(|(screen, _, _)| (screen.width(), screen.height()));
            (animator.frames.len(), size)
        };
        let (_, duration) = self.time_info();
        let progress = self.export_job.as_ref().map(|job| {
            let progress = &job.progress;
            (
                progress.frame.load(Ordering::Relaxed),
                progress.total.load(Ordering::Relaxed),
                progress.encoding.load(Ordering::Relaxed),
            )
        });
        let error = self.export_error.clone();
        let dialog = self.export_dialog.as_mut().unwrap();
        let mut browse = false;
        let export = fl!("menu-export").trim_end_matches(['…', '.']).to_string();
        let buttons = if progress.is_some() {
            vec![DialogButton::cancel(labels::cancel(), Action::Stop)]
        } else {
            vec![
                DialogButton::cancel(labels::cancel(), Action::Close),
                DialogButton::primary(export, Action::Export).enabled(frames > 0 && !dialog.path.trim().is_empty()),
            ]
        };
        let response = appearance::Dialog::new("animation-export").size(DialogSize::Medium).show(context, |frame| {
            frame.content(|ui| {
                if blocked {
                    ui.disable();
                }
                ui.add_enabled_ui(progress.is_none(), |ui| {
                    appearance::group(ui, "", |ui| {
                        appearance::combo_row(ui, &fl!("animation-export-format"), dialog.format.name(), |ui| {
                            for format in ExportFormat::ALL {
                                if ui.selectable_value(&mut dialog.format, format, format.name()).changed() && !dialog.path.trim().is_empty() {
                                    dialog.path = PathBuf::from(dialog.path.trim()).with_extension(format.extension()).display().to_string();
                                }
                            }
                        });
                        appearance::form_row(ui, &fl!("animation-export-path"), |ui| {
                            ui.horizontal(|ui| {
                                let label = fl!("animation-export-browse");
                                let width = (ui.available_width() - 96.0).max(80.0);
                                ui.add(
                                    appearance::text_edit(&mut dialog.path)
                                        .desired_width(width)
                                        .hint_text(fl!("animation-export-no-path")),
                                );
                                browse = ui.button(label).clicked();
                            });
                        });
                    });
                });
                ui.add_space(4.0);
                let summary = match size {
                    Some((width, height)) => fl!(
                        "animation-export-summary",
                        frames = frames,
                        width = width,
                        height = height,
                        duration = format_time(duration)
                    ),
                    None => fl!("animation-export-no-frames"),
                };
                ui.label(egui::RichText::new(summary).weak().size(12.0));
                if let Some((current, total, encoding)) = progress {
                    ui.add_space(8.0);
                    let fraction = if encoding { 1.0 } else { current as f32 / total.max(1) as f32 };
                    let text = if encoding {
                        fl!("animation-export-encoding")
                    } else {
                        fl!("animation-export-exporting-frame", current = current, total = total)
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(text).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{}%", (fraction * 100.0).round())).size(12.0).weak());
                        });
                    });
                    ui.add(egui::ProgressBar::new(fraction).desired_height(8.0).animate(encoding));
                }
                if let Some(error) = &error {
                    ui.add_space(8.0);
                    ui.colored_label(DANGER, error);
                }
            });
            frame.buttons(buttons);
        });
        if blocked {
            return;
        }
        match response.action {
            Some(Action::Stop) => {
                if let Some(job) = &self.export_job {
                    job.progress.cancelled.store(true, Ordering::Relaxed);
                }
            }
            Some(Action::Export) => {
                let mut path = PathBuf::from(dialog.path.trim());
                if path.extension().is_none() {
                    path.set_extension(dialog.format.extension());
                    dialog.path = path.display().to_string();
                }
                self.request = Some(Request::Export(path, dialog.format));
            }
            Some(Action::Close) => {
                self.export_dialog = None;
                self.export_error = None;
            }
            None if response.dismissed && progress.is_none() => {
                self.export_dialog = None;
                self.export_error = None;
            }
            None if browse => self.request = Some(Request::Browse(dialog.format)),
            None => {}
        }
    }
}

fn speed_label(speed: f32) -> String {
    format!("{speed}×")
}

/// `MM:SS.t`
fn format_time(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!("{:02}:{:02}.{}", seconds / 60, seconds % 60, (milliseconds % 1000) / 100)
}

/// Monospace caption on a translucent dark plate, drawn over the preview.
fn overlay_label(painter: &egui::Painter, anchor: egui::Pos2, align: egui::Align, text: String) {
    let galley = painter.layout_no_wrap(text, egui::FontId::monospace(12.0), Color32::WHITE);
    let size = galley.size() + egui::vec2(20.0, 8.0);
    let left = if align == egui::Align::RIGHT { anchor.x - size.x } else { anchor.x };
    let rect = egui::Rect::from_min_size(egui::pos2(left, anchor.y), size);
    painter.rect_filled(rect, 4, Color32::from_black_alpha(128));
    painter.galley(rect.min + egui::vec2(10.0, 4.0), galley, Color32::WHITE);
}

/// Framed square transport button, filled with the accent color while `active`.
fn transport_button(icons: &mut Icons, ui: &mut egui::Ui, icon: &str, tooltip: &str, enabled: bool, active: bool) -> egui::Response {
    let image = icons.image(ui, icon, 18.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(TRANSPORT_BUTTON), sense);
    let visuals = ui.visuals();
    let (fill, stroke, tint) = if active && enabled {
        (PRIMARY, PRIMARY, Color32::WHITE)
    } else if !enabled {
        (
            visuals.widgets.inactive.weak_bg_fill.gamma_multiply(0.5),
            visuals.widgets.noninteractive.bg_stroke.color,
            visuals.weak_text_color().gamma_multiply(0.5),
        )
    } else if response.is_pointer_button_down_on() {
        (
            visuals.widgets.active.weak_bg_fill,
            visuals.widgets.active.bg_stroke.color,
            visuals.text_color(),
        )
    } else if response.hovered() {
        (
            visuals.widgets.hovered.weak_bg_fill,
            visuals.widgets.hovered.bg_stroke.color,
            visuals.text_color(),
        )
    } else {
        (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.widgets.noninteractive.bg_stroke.color,
            visuals.text_color(),
        )
    };
    ui.painter().rect(rect, 6, fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
    image
        .tint(tint)
        .paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(18.0)));
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, tooltip));
    response.on_hover_text(tooltip)
}

/// Round play/pause button: green to start playback, red while playing.
fn play_button(icons: &mut Icons, ui: &mut egui::Ui, playing: bool, enabled: bool) -> egui::Response {
    let image = icons.image(ui, if playing { "pause" } else { "play" }, 22.0);
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(PLAY_BUTTON), sense);
    let base = if !enabled {
        ui.visuals().widgets.inactive.weak_bg_fill
    } else if playing {
        DANGER
    } else {
        PLAY
    };
    let fill = if enabled && response.hovered() { base.gamma_multiply(1.15) } else { base };
    let painter = ui.painter();
    painter.circle_filled(rect.center() + egui::vec2(0.0, 2.0), PLAY_BUTTON / 2.0, Color32::from_black_alpha(60));
    painter.circle_filled(rect.center(), PLAY_BUTTON / 2.0, fill);
    let tint = if enabled { Color32::WHITE } else { ui.visuals().weak_text_color() };
    // The play triangle looks centred when nudged right a little.
    let offset = if playing { 0.0 } else { 1.5 };
    image.tint(tint).paint_at(
        ui,
        egui::Rect::from_center_size(rect.center() + egui::vec2(offset, 0.0), egui::Vec2::splat(22.0)),
    );
    let label = fl!("animation-play-pause");
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, playing, &label));
    response.on_hover_text(fl!("animation-play-pause"))
}

/// Solarized accents, which read well on both the dark and the light editor background.
struct SyntaxColors {
    text: Color32,
    keyword: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    function: Color32,
}

impl SyntaxColors {
    fn new(visuals: &egui::Visuals) -> Self {
        Self {
            text: visuals.text_color(),
            keyword: Color32::from_rgb(133, 153, 0),
            string: Color32::from_rgb(42, 161, 152),
            number: Color32::from_rgb(108, 113, 196),
            comment: if visuals.dark_mode {
                Color32::from_rgb(110, 128, 134)
            } else {
                Color32::from_rgb(120, 130, 136)
            },
            function: Color32::from_rgb(38, 139, 210),
        }
    }
}

const LUA_KEYWORDS: [&str; 19] = [
    "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local", "not", "or", "repeat", "return", "then", "until", "while",
];

/// Level of a Lua long bracket (`[[`, `[==[`) at the start of `text`.
fn long_bracket(text: &str) -> Option<usize> {
    let rest = text.strip_prefix('[')?;
    let level = rest.bytes().take_while(|byte| *byte == b'=').count();
    rest[level..].starts_with('[').then_some(level)
}

/// End of the long string or comment opened at `start` with the given bracket level.
fn long_bracket_end(text: &str, start: usize, level: usize) -> usize {
    let close = format!("]{}]", "=".repeat(level));
    let open = start + level + 2;
    text[open.min(text.len())..].find(&close).map_or(text.len(), |end| open + end + close.len())
}

/// Splits Lua source into colored spans that together cover the whole text.
fn lua_spans<'a>(text: &'a str, colors: &SyntaxColors) -> Vec<(&'a str, Color32)> {
    let bytes = text.as_bytes();
    let mut spans: Vec<(usize, usize, Color32)> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let byte = bytes[index];
        let color = if text[index..].starts_with("--") {
            index = match long_bracket(&text[index + 2..]) {
                Some(level) => long_bracket_end(text, index + 2, level),
                None => text[index..].find('\n').map_or(text.len(), |end| index + end),
            };
            colors.comment
        } else if byte == b'"' || byte == b'\'' {
            index += 1;
            while index < bytes.len() && bytes[index] != byte && bytes[index] != b'\n' {
                index += if bytes[index] == b'\\' { 2 } else { 1 };
            }
            index = (index + usize::from(index < bytes.len() && bytes[index] == byte)).min(bytes.len());
            colors.string
        } else if let Some(level) = long_bracket(&text[index..]) {
            index = long_bracket_end(text, index, level);
            colors.string
        } else if byte.is_ascii_digit() || (byte == b'.' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)) {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || bytes[index] == b'.'
                    || (matches!(bytes[index], b'+' | b'-') && matches!(bytes[index - 1], b'e' | b'E' | b'p' | b'P')))
            {
                index += 1;
            }
            colors.number
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
                index += 1;
            }
            let word = &text[start..index];
            if LUA_KEYWORDS.contains(&word) {
                colors.keyword
            } else if matches!(word, "true" | "false" | "nil") {
                colors.number
            } else if text[index..].trim_start_matches([' ', '\t']).starts_with('(') {
                colors.function
            } else {
                colors.text
            }
        } else {
            index += text[index..].chars().next().map_or(1, char::len_utf8);
            colors.text
        };
        match spans.last_mut() {
            Some((_, end, last)) if *last == color && *end == start && color == colors.text => *end = index,
            _ => spans.push((start, index, color)),
        }
    }
    spans.into_iter().map(|(start, end, color)| (&text[start..end], color)).collect()
}

impl Drop for AnimationEditor {
    fn drop(&mut self) {
        if let Some(job) = &self.export_job {
            job.progress.cancelled.store(true, Ordering::Relaxed);
        }
    }
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
    fn lua_highlighting_colors_keywords_strings_numbers_and_comments() {
        let colors = SyntaxColors::new(&egui::Visuals::dark());
        let source = "local s = \"x\" -- note\nbuf:print(12) --[[ a\nb ]] t = [[ long ]]";
        let spans = lua_spans(source, &colors);
        let color_of = |text: &str| spans.iter().find(|(span, _)| *span == text).map(|(_, color)| *color);
        assert_eq!(color_of("local"), Some(colors.keyword));
        assert_eq!(color_of("\"x\""), Some(colors.string));
        assert_eq!(color_of("-- note"), Some(colors.comment));
        assert_eq!(color_of("print"), Some(colors.function));
        assert_eq!(color_of("12"), Some(colors.number));
        assert_eq!(color_of("--[[ a\nb ]]"), Some(colors.comment));
        assert_eq!(color_of("[[ long ]]"), Some(colors.string));
        assert_eq!(spans.iter().map(|(span, _)| *span).collect::<String>(), source);
        let unicode = "x = \"░▒\\\"\" y";
        assert_eq!(lua_spans(unicode, &colors).iter().map(|(span, _)| *span).collect::<String>(), unicode);
    }

    #[test]
    fn time_is_the_sum_of_frame_delays() {
        assert_eq!(format_time(0), "00:00.0");
        assert_eq!(format_time(65_430), "01:05.4");
        let mut editor = AnimationEditor::new();
        for delay in [100, 250, 400] {
            editor.animator.lock().frames.push((
                Box::new(icy_engine::TextScreen::new((10, 3))),
                icy_engine_scripting::MonitorSettings::neutral(),
                delay,
            ));
        }
        editor.frame = 2;
        assert_eq!(editor.time_info(), (350, 750));
    }

    #[test]
    fn picked_export_paths_select_the_format() {
        let mut editor = AnimationEditor::new();
        editor.open_export_dialog();
        editor.set_export_path(PathBuf::from("/tmp/demo.ivf"));
        assert_eq!(editor.export_dialog.as_ref().unwrap().format, ExportFormat::Av1);
        editor.set_export_path(PathBuf::from("/tmp/demo"));
        assert_eq!(editor.export_dialog.as_ref().unwrap().path, "/tmp/demo.ivf");
    }
}
