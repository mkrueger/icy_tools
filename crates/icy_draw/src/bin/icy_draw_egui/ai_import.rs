//! Local image-to-ANSI import. The dialog and chat use the same bounded converter.

use std::{
    io::Cursor,
    path::Path,
    sync::{mpsc, Arc},
};

use eframe::egui;
use icy_draw::fl;
use icy_engine::{IceMode, Rectangle, TextBuffer};
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use image::{imageops, ImageFormat};
use parking_lot::Mutex;
use serde_json::json;

use super::ai_chat::{
    canvas::Draft,
    image_attachment::{self, ReferenceImage},
};

pub enum Action {
    Browse,
    Accept(Box<TextBuffer>),
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Preset {
    Scene,
    Shaded,
}

impl Preset {
    fn label(self) -> String {
        match self {
            Self::Scene => fl!("ai-import-scene"),
            Self::Shaded => fl!("ai-import-shaded"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    Crop,
    Contain,
    Stretch,
}

impl Fit {
    fn label(self) -> String {
        match self {
            Self::Crop => fl!("ai-import-crop"),
            Self::Contain => fl!("ai-import-contain"),
            Self::Stretch => fl!("ai-import-stretch"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glyphs {
    HalfBlocks,
    Blocks,
    Full,
    Ascii,
}

impl Glyphs {
    fn mode(self) -> &'static str {
        match self {
            Self::HalfBlocks => "half_pixels",
            Self::Blocks => "blocks",
            Self::Full => "full",
            Self::Ascii => "ascii",
        }
    }

    fn label(self) -> String {
        match self {
            Self::HalfBlocks => fl!("ai-import-half-blocks"),
            Self::Blocks => fl!("ai-import-blocks"),
            Self::Full => fl!("ai-import-full-glyphs"),
            Self::Ascii => fl!("ai-import-ascii"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Options {
    columns: i32,
    rows: i32,
    preset: Preset,
    fit: Fit,
    focus: egui::Rect,
    ice: bool,
    spacing: bool,
    aspect: bool,
    glyphs: Glyphs,
    dither: bool,
    brightness: f64,
    contrast: f64,
    saturation: f64,
    shade_penalty: f64,
    coherence: f64,
    lightness_levels: u8,
    hue_families: bool,
    local_contrast: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 50,
            preset: Preset::Scene,
            fit: Fit::Crop,
            focus: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ice: false,
            spacing: false,
            aspect: false,
            glyphs: Glyphs::Blocks,
            dither: false,
            brightness: 0.03,
            contrast: 1.2,
            saturation: 2.0,
            shade_penalty: 0.10,
            coherence: 0.0015,
            lightness_levels: 0,
            hue_families: true,
            local_contrast: 0.75,
        }
    }
}

impl Options {
    fn select_preset(&mut self, preset: Preset) {
        self.preset = preset;
        self.brightness = if preset == Preset::Scene { 0.03 } else { 0.0 };
        self.contrast = if preset == Preset::Scene { 1.2 } else { 1.0 };
        self.lightness_levels = if preset == Preset::Shaded && self.glyphs != Glyphs::HalfBlocks {
            8
        } else {
            0
        };
        self.saturation = 2.0;
        self.shade_penalty = if preset == Preset::Scene { 0.10 } else { 1.0 };
        self.coherence = 0.0015;
        self.hue_families = true;
        self.local_contrast = if preset == Preset::Scene { 0.75 } else { 0.5 };
    }

    fn validate(&self) -> Result<(), String> {
        if !(1..=160).contains(&self.columns) || !(1..=200).contains(&self.rows) || self.columns * self.rows > 8000 {
            return Err(fl!("ai-import-invalid-size"));
        }
        if !self.focus.is_finite()
            || self.focus.width() <= 0.0
            || self.focus.height() <= 0.0
            || !egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)).contains_rect(self.focus)
        {
            return Err(fl!("ai-import-invalid-crop"));
        }
        Ok(())
    }

    fn buffer(&self) -> TextBuffer {
        let mut buffer = TextBuffer::new((self.columns, self.rows));
        buffer.terminal_state.is_terminal_buffer = false;
        buffer.ice_mode = if self.ice { IceMode::Ice } else { IceMode::Blink };
        buffer.set_use_letter_spacing(self.spacing);
        buffer.set_use_aspect_ratio(self.aspect);
        buffer
    }

    fn crop(&self, source: &ReferenceImage) -> (u32, u32, u32, u32) {
        let left = (self.focus.left() * source.width as f32).floor() as u32;
        let top = (self.focus.top() * source.height as f32).floor() as u32;
        let mut width = ((self.focus.right() * source.width as f32).ceil() as u32).min(source.width) - left;
        let mut height = ((self.focus.bottom() * source.height as f32).ceil() as u32).min(source.height) - top;
        let mut x = left;
        let mut y = top;
        if self.fit == Fit::Crop {
            let buffer = self.buffer();
            let aspect = if self.aspect { buffer.get_aspect_ratio_stretch_factor() } else { 1.0 };
            let target = self.columns as f64 * if self.spacing { 9.0 } else { 8.0 } / (self.rows as f64 * 16.0 * f64::from(aspect));
            if f64::from(width) / f64::from(height) > target {
                let cropped = (f64::from(height) * target).round().clamp(1.0, f64::from(width)) as u32;
                x += (width - cropped) / 2;
                width = cropped;
            } else {
                let cropped = (f64::from(width) / target).round().clamp(1.0, f64::from(height)) as u32;
                y += (height - cropped) / 2;
                height = cropped;
            }
        }
        (x, y, width, height)
    }
}

struct Converted {
    buffer: TextBuffer,
    preview: egui::ColorImage,
}

fn convert(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    options.validate()?;
    let (x, y, width, height) = options.crop(source);
    let cropped = imageops::crop_imm(&source.pixels()?, x, y, width, height).to_image();
    let mut png = Cursor::new(Vec::new());
    cropped.write_to(&mut png, ImageFormat::Png).map_err(|error| error.to_string())?;
    let reference = image_attachment::prepare(&source.name, &png.into_inner())?;
    let mut draft = Draft::new("ANSI/ASCII", options.buffer(), 0, None);
    draft.begin_turn(Some(reference));
    draft.convert_image(&json!({
        "preset": "scene", "mode": options.glyphs.mode(), "dither": options.dither,
        "fit": if options.fit == Fit::Contain { "contain" } else { "stretch" },
        "brightness": options.brightness, "contrast": options.contrast, "saturation": options.saturation,
        "shade_penalty": options.shade_penalty, "coherence": options.coherence,
        "lightness_levels": options.lightness_levels, "hue_families": options.hue_families,
        "local_contrast": options.local_contrast,
    }))?;
    let buffer = draft.buffer;
    let (size, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, options.columns, options.rows).into(), false);
    let preview = if options.aspect {
        let height = (size.height as f32 * buffer.get_aspect_ratio_stretch_factor()).round().max(1.0) as u32;
        let raw = image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or("Invalid rendered image dimensions")?;
        let corrected = imageops::resize(&raw, size.width as u32, height, imageops::FilterType::Nearest);
        egui::ColorImage::from_rgba_unmultiplied([size.width as usize, height as usize], corrected.as_raw())
    } else {
        egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &pixels)
    };
    Ok(Converted { buffer, preview })
}

enum Completed {
    Source(ReferenceImage),
    Result(Converted),
}

type Worker = Arc<Mutex<mpsc::Receiver<Result<Completed, String>>>>;

#[derive(Clone, Default)]
pub struct ImportDialog {
    source: Option<ReferenceImage>,
    source_texture: Option<egui::TextureHandle>,
    result: Option<Arc<Converted>>,
    result_texture: Option<egui::TextureHandle>,
    options: Options,
    worker: Option<Worker>,
    error: Option<String>,
    drag_start: Option<egui::Pos2>,
}

impl ImportDialog {
    fn start(&mut self, context: &egui::Context, work: impl FnOnce() -> Result<Completed, String> + Send + 'static) {
        self.invalidate();
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(Arc::new(Mutex::new(receiver)));
        let context = context.clone();
        std::thread::spawn(move || {
            let _ = sender.send(work());
            context.request_repaint();
        });
    }

    pub fn load(&mut self, context: &egui::Context, path: &Path) {
        self.source = None;
        self.source_texture = None;
        self.options.focus = Options::default().focus;
        self.drag_start = None;
        let path = path.to_path_buf();
        self.start(context, move || {
            image_attachment::read_drop(egui::DroppedFile {
                path: Some(path),
                ..Default::default()
            })
            .map(Completed::Source)
        });
    }

    fn invalidate(&mut self) {
        self.result = None;
        self.result_texture = None;
        self.error = None;
    }

    fn poll(&mut self) {
        let Some(worker) = &self.worker else { return };
        let message = worker.lock().try_recv();
        let result = match message {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(fl!("ai-import-worker-failed")),
        };
        self.worker = None;
        match result {
            Ok(Completed::Source(source)) => self.source = Some(source),
            Ok(Completed::Result(result)) => self.result = Some(Arc::new(result)),
            Err(error) => {
                log::warn!("Image import failed: {error}");
                self.error = Some(error);
            }
        }
    }

    fn start_conversion(&mut self, context: &egui::Context) {
        let Some(source) = self.source.clone() else {
            self.error = Some(fl!("ai-import-select-source"));
            return;
        };
        if let Err(error) = self.options.validate() {
            self.error = Some(error);
            return;
        }
        let options = self.options.clone();
        self.start(context, move || convert(&source, &options).map(Completed::Result));
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        #[derive(Clone, Copy)]
        enum Button {
            Convert,
            Accept,
            Cancel,
        }
        self.poll();
        let busy = self.worker.is_some();
        let before = self.options.clone();
        let mut browse = false;
        let response = appearance::Dialog::new("ai-import")
            .title(fl!("ai-import-title"))
            .size(DialogSize::Width(1120.0))
            .fixed_height(900.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    ui.add_enabled_ui(!blocked && !busy, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            browse = ui.button(fl!("ai-import-browse")).clicked();
                            if let Some(source) = &self.source {
                                ui.label(format!("{} ({} x {})", source.name, source.width, source.height));
                                if source.original_size != (source.width, source.height) {
                                    ui.weak(fl!("ai-import-normalized", width = source.original_size.0, height = source.original_size.1));
                                }
                            } else {
                                ui.weak(fl!("ai-import-select-source"));
                            }
                        });
                        ui.weak(fl!("ai-import-local"));
                        ui.separator();
                        self.controls(ui);
                    });
                    ui.separator();
                    let controls_height = ui.cursor().top() - ui.max_rect().top();
                    let height = (ui.clip_rect().height() - controls_height - 64.0).clamp(80.0, 450.0);
                    ui.columns(2, |columns| {
                        columns[0].label(fl!("ai-import-source"));
                        columns[1].label(fl!("ai-import-result"));
                        columns[0].add_enabled_ui(!blocked && !busy, |ui| self.source_preview(ui, height));
                        self.result_preview(&mut columns[1], height);
                    });
                    if self.source.is_some() {
                        ui.weak(fl!("ai-import-focus-hint"));
                    }
                    if busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(fl!("ai-import-working"));
                        });
                    } else if let Some(error) = &self.error {
                        ui.colored_label(icy_engine_gui::egui::dialog::DANGER, error);
                    } else if let Err(error) = self.options.validate() {
                        ui.colored_label(icy_engine_gui::egui::dialog::DANGER, error);
                    }
                });
                if self.options != before {
                    self.invalidate();
                }
                dialog.buttons([
                    DialogButton::secondary(fl!("ai-import-convert"), Button::Convert)
                        .leading()
                        .enabled(!blocked && !busy && self.source.is_some() && self.options.validate().is_ok()),
                    DialogButton::cancel(labels::cancel(), Button::Cancel).enabled(!blocked),
                    DialogButton::primary(fl!("ai-import-accept"), Button::Accept).enabled(!blocked && !busy && self.result.is_some()),
                ]);
            });
        if blocked {
            return None;
        }
        match response.action {
            Some(Button::Accept) if !busy => self.result.as_ref().map(|result| Action::Accept(Box::new(result.buffer.clone()))),
            Some(Button::Convert) if !busy => {
                self.start_conversion(context);
                None
            }
            Some(Button::Cancel) => Some(Action::Cancel),
            _ if response.dismissed => Some(Action::Cancel),
            _ if browse && !busy => Some(Action::Browse),
            _ => None,
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(fl!("ai-import-style"));
            for preset in [Preset::Scene, Preset::Shaded] {
                if ui.selectable_label(self.options.preset == preset, preset.label()).clicked() {
                    self.options.select_preset(preset);
                }
            }
            ui.separator();
            for (columns, rows) in [(80, 25), (80, 50)] {
                if ui
                    .selectable_label((self.options.columns, self.options.rows) == (columns, rows), format!("{columns} x {rows}"))
                    .clicked()
                {
                    self.options.columns = columns;
                    self.options.rows = rows;
                }
            }
            ui.label(fl!("ai-import-columns"));
            ui.add(egui::DragValue::new(&mut self.options.columns).range(1..=160));
            ui.label(fl!("ai-import-rows"));
            ui.add(egui::DragValue::new(&mut self.options.rows).range(1..=200));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(fl!("ai-import-fit"));
            for fit in [Fit::Crop, Fit::Contain, Fit::Stretch] {
                ui.selectable_value(&mut self.options.fit, fit, fit.label());
            }
            if ui.button(fl!("ai-import-reset-focus")).clicked() {
                self.options.focus = Options::default().focus;
            }
            ui.checkbox(&mut self.options.ice, fl!("ai-import-ice"));
            ui.checkbox(&mut self.options.spacing, fl!("ai-import-spacing"));
            ui.checkbox(&mut self.options.aspect, fl!("ai-import-aspect"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(fl!("ai-import-glyphs"));
            for glyphs in [Glyphs::HalfBlocks, Glyphs::Blocks, Glyphs::Full, Glyphs::Ascii] {
                if ui.selectable_value(&mut self.options.glyphs, glyphs, glyphs.label()).changed() {
                    self.options.dither = glyphs == Glyphs::HalfBlocks;
                    if glyphs == Glyphs::HalfBlocks {
                        self.options.lightness_levels = 0;
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.options.hue_families, fl!("ai-import-hue-families"));
            ui.checkbox(&mut self.options.dither, fl!("ai-import-dither"));
        });
        egui::CollapsingHeader::new(fl!("ai-import-tuning")).show(ui, |ui| {
            ui.add(egui::Slider::new(&mut self.options.brightness, -0.25..=0.25).text(fl!("ai-import-brightness")));
            ui.add(egui::Slider::new(&mut self.options.contrast, 0.5..=2.0).text(fl!("ai-import-contrast")));
            ui.add(egui::Slider::new(&mut self.options.local_contrast, 0.0..=2.0).text(fl!("ai-import-local-contrast")));
            ui.add(egui::Slider::new(&mut self.options.saturation, 0.0..=2.0).text(fl!("ai-import-saturation")));
            ui.add(egui::Slider::new(&mut self.options.shade_penalty, 0.0..=2.0).text(fl!("ai-import-shading")));
            ui.add(egui::Slider::new(&mut self.options.coherence, 0.0..=0.02).text(fl!("ai-import-coherence")));
            ui.horizontal(|ui| {
                let mut enabled = self.options.lightness_levels != 0;
                if ui.checkbox(&mut enabled, fl!("ai-import-levels")).changed() {
                    self.options.lightness_levels = if enabled { 5 } else { 0 };
                }
                if enabled {
                    ui.add(egui::DragValue::new(&mut self.options.lightness_levels).range(2..=16));
                }
            });
        });
    }

    fn source_preview(&mut self, ui: &mut egui::Ui, height: f32) {
        let Some(source) = &self.source else {
            ui.allocate_space(egui::vec2(ui.available_width(), height));
            return;
        };
        let texture = self
            .source_texture
            .get_or_insert_with(|| ui.ctx().load_texture("import-source", source.color_image(), egui::TextureOptions::LINEAR));
        let size = fit_preview(texture.size_vec2(), egui::vec2(ui.available_width(), height));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());
        egui::Image::new(&*texture).paint_at(ui, rect);
        let normalize = |point: egui::Pos2| {
            let delta = (point - rect.min) / rect.size();
            egui::pos2(delta.x.clamp(0.0, 1.0), delta.y.clamp(0.0, 1.0))
        };
        if response.drag_started() {
            self.drag_start = ui.input(|input| input.pointer.press_origin()).map(normalize);
        }
        let dragging = self
            .drag_start
            .zip(response.interact_pointer_pos())
            .map(|(start, end)| egui::Rect::from_two_pos(start, normalize(end)));
        if response.drag_stopped() {
            if let Some(rect) = dragging.filter(|rect| rect.width() * source.width as f32 >= 1.0 && rect.height() * source.height as f32 >= 1.0) {
                self.options.focus = rect;
            }
            self.drag_start = None;
        }
        let focus = dragging.unwrap_or(self.options.focus);
        let project = |focus: egui::Rect| egui::Rect::from_min_max(rect.min + focus.min.to_vec2() * rect.size(), rect.min + focus.max.to_vec2() * rect.size());
        ui.painter()
            .rect_stroke(project(focus), 0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
        if self.options.validate().is_ok() {
            let (x, y, width, height) = self.options.crop(source);
            let rect = egui::Rect::from_min_max(
                egui::pos2(x as f32 / source.width as f32, y as f32 / source.height as f32),
                egui::pos2((x + width) as f32 / source.width as f32, (y + height) as f32 / source.height as f32),
            );
            ui.painter()
                .rect_stroke(project(rect), 0, egui::Stroke::new(2.0, appearance::PRIMARY), egui::StrokeKind::Inside);
        }
    }

    fn result_preview(&mut self, ui: &mut egui::Ui, height: f32) {
        if let Some(result) = &self.result {
            let texture = self
                .result_texture
                .get_or_insert_with(|| ui.ctx().load_texture("import-result", result.preview.clone(), egui::TextureOptions::NEAREST));
            let size = fit_preview(texture.size_vec2(), egui::vec2(ui.available_width(), height));
            ui.add(egui::Image::new(&*texture).fit_to_exact_size(size));
        } else {
            ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| {
                ui.centered_and_justified(|ui| ui.weak(fl!("ai-import-preview-hint")));
            });
        }
    }
}

fn fit_preview(size: egui::Vec2, available: egui::Vec2) -> egui::Vec2 {
    size * (available.x / size.x).min(available.y / size.y).max(0.01)
}

#[cfg(test)]
mod tests {
    use super::super::{
        tests::{click_text, frame, use_english, Gpu},
        Dialog, DrawApp, FileAction, NewKind, Picked,
    };
    use super::*;
    use icy_engine::{Position, Size, TextPane};
    use std::time::{Duration, Instant};

    fn source() -> ReferenceImage {
        let image = image::RgbaImage::from_fn(80, 100, |x, y| {
            let face = (i64::from(x) - 40).pow(2) * 2 + (i64::from(y) - 45).pow(2) < 1100;
            image::Rgba(if face {
                [(130 + x) as u8, (70 + y) as u8, 70, 255]
            } else {
                [10, 20, 65, 255]
            })
        });
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        image_attachment::prepare("portrait.png", &png.into_inner()).unwrap()
    }

    fn ready_dialog() -> ImportDialog {
        let source = source();
        let options = Options {
            columns: 8,
            rows: 5,
            ..Options::default()
        };
        let converted = convert(&source, &options).unwrap();
        ImportDialog {
            source: Some(source),
            options,
            result: Some(Arc::new(converted)),
            ..ImportDialog::default()
        }
    }

    fn wait(app: &mut DrawApp, context: &egui::Context, size: egui::Vec2) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            frame(context, app, size, vec![]);
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog closed")
            };
            if dialog.worker.is_none() {
                break;
            }
            assert!(Instant::now() < deadline, "image worker did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
        for _ in 0..3 {
            frame(context, app, size, vec![]);
        }
    }

    #[test]
    fn image_import_geometry_matches_fit_focus_and_display_aspect() {
        use_english();
        let source = source();
        let mut options = Options {
            columns: 10,
            rows: 10,
            ..Options::default()
        };
        assert_eq!(options.crop(&source), (15, 0, 50, 100));
        options.focus = egui::Rect::from_min_max(egui::pos2(0.25, 0.2), egui::pos2(0.75, 0.8));
        assert_eq!(options.crop(&source), (25, 20, 30, 60));
        for fit in [Fit::Contain, Fit::Stretch] {
            options.fit = fit;
            assert_eq!(options.crop(&source), (20, 20, 40, 60));
        }
        options.fit = Fit::Crop;
        options.spacing = true;
        assert_eq!(options.crop(&source).2, 34);
        options.aspect = true;
        assert!(options.crop(&source).2 < 34);
        options.focus = egui::Rect::from_min_max(egui::pos2(-0.1, 0.0), egui::pos2(1.0, 1.0));
        assert!(options.validate().is_err());
        options.focus = Options::default().focus;
        options.columns = 160;
        options.rows = 200;
        assert!(options.validate().is_err());
    }

    #[test]
    fn image_import_presets_generate_legal_editable_ansi_in_each_fit() {
        use_english();
        for preset in [Preset::Scene, Preset::Shaded] {
            for fit in [Fit::Crop, Fit::Contain, Fit::Stretch] {
                let mut options = Options {
                    columns: 8,
                    rows: 5,
                    fit,
                    ..Options::default()
                };
                options.select_preset(preset);
                let converted = convert(&source(), &options).unwrap();
                assert_eq!(converted.buffer.size(), Size::new(8, 5));
                assert_eq!(converted.buffer.buffer_type, icy_engine::BufferType::CP437);
                assert_eq!(converted.preview.size, [64, 80]);
                assert!(!converted.buffer.terminal_state.is_terminal_buffer);
                assert!(converted.preview.pixels.iter().any(|pixel| *pixel != egui::Color32::BLACK));
                for row in 0..5 {
                    for column in 0..8 {
                        let cell = converted.buffer.char_at(Position::new(column, row));
                        assert!(cell.attribute.foreground() < 16 && cell.attribute.background() < 8);
                        assert!(!cell.attribute.is_blinking());
                    }
                }
            }
        }
        let mut options = Options::default();
        options.select_preset(Preset::Shaded);
        assert!(options.hue_families);
        assert_eq!(options.lightness_levels, 8);
        options.select_preset(Preset::Scene);
        assert!(options.hue_families);
    }

    #[test]
    #[ignore = "requires ICY_IMPORT_REFERENCE and ICY_IMPORT_OUTPUT for local visual evaluation"]
    fn image_import_reference_previews() {
        let input = std::env::var("ICY_IMPORT_REFERENCE").expect("path to a local reference picture");
        let output = std::path::PathBuf::from(std::env::var("ICY_IMPORT_OUTPUT").expect("preview output directory"));
        std::fs::create_dir_all(&output).unwrap();
        let source = image_attachment::read_drop(egui::DroppedFile {
            path: Some(input.into()),
            ..Default::default()
        })
        .unwrap();
        for (name, preset, glyphs) in [
            ("scene-blocks", Preset::Scene, Glyphs::Blocks),
            ("shaded-blocks", Preset::Shaded, Glyphs::Blocks),
            ("shaded-full", Preset::Shaded, Glyphs::Full),
            ("half-blocks", Preset::Shaded, Glyphs::HalfBlocks),
            ("ascii", Preset::Shaded, Glyphs::Ascii),
        ] {
            let mut options = Options {
                rows: 25,
                fit: Fit::Stretch,
                ice: false,
                glyphs,
                ..Default::default()
            };
            options.select_preset(preset);
            options.hue_families = true;
            options.dither = glyphs == Glyphs::HalfBlocks;
            let result = convert(&source, &options).unwrap();
            let pixels: Vec<_> = result.preview.pixels.iter().flat_map(|pixel| pixel.to_array()).collect();
            image::save_buffer(
                output.join(format!("{name}.png")),
                &pixels,
                result.preview.size[0] as u32,
                result.preview.size[1] as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            let bytes = icy_engine::formats::FileFormat::Ansi
                .to_bytes(&result.buffer, &icy_engine::formats::SaveOptions::default())
                .unwrap();
            std::fs::write(output.join(format!("{name}.ans")), bytes).unwrap();
            if glyphs == Glyphs::HalfBlocks {
                if let Some(path) = std::env::var_os("ICY_IMPORT_HALF_REFERENCE") {
                    let expected = icy_engine::formats::FileFormat::Ansi.load(Path::new(&path), None).unwrap().screen.buffer;
                    assert_eq!(expected.width(), options.columns);
                    let (size, expected) = expected.render_to_rgba(&Rectangle::from(0, 0, options.columns, options.rows).into(), false);
                    assert_eq!(result.preview.size, [size.width as usize, size.height as usize]);
                    let squared: f64 = pixels
                        .chunks_exact(4)
                        .zip(expected.chunks_exact(4))
                        .flat_map(|(a, b)| (0..3).map(move |c| (f64::from(a[c]) - f64::from(b[c])).powi(2)))
                        .sum();
                    let rmse = (squared / (result.preview.pixels.len() * 3) as f64).sqrt() / 255.0;
                    eprintln!("Half-pixel reference RGB RMSE: {rmse:.6}");
                    assert!(rmse < 0.08, "half-pixel reference deviation exceeds the portrait regression limit");
                }
            }
        }
    }

    #[test]
    fn image_import_glyph_controls_invalidate_preview_and_limit_output() {
        use_english();
        for glyphs in [Glyphs::HalfBlocks, Glyphs::Ascii] {
            let context = egui::Context::default();
            appearance::apply(&context);
            let size = egui::vec2(1280.0, 900.0);
            let mut app = DrawApp::new();
            app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
            for _ in 0..3 {
                frame(&context, &mut app, size, vec![]);
            }
            click_text(&context, &mut app, size, &glyphs.label());
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog")
            };
            assert_eq!(dialog.options.glyphs, glyphs);
            assert_eq!(dialog.options.dither, glyphs == Glyphs::HalfBlocks);
            assert!(dialog.result.is_none());
            click_text(&context, &mut app, size, &fl!("ai-import-convert"));
            wait(&mut app, &context, size);
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog")
            };
            let result = dialog.result.as_ref().expect("glyph-constrained conversion");
            let mut halves = 0;
            let mut characters = 0;
            for y in 0..5 {
                for x in 0..8 {
                    let cell = result.buffer.char_at(Position::new(x, y));
                    if glyphs == Glyphs::HalfBlocks {
                        assert!([32, 219, 220, 223].contains(&(cell.ch as u32)));
                        halves += usize::from(matches!(cell.ch as u32, 220 | 223) && cell.attribute.foreground() != cell.attribute.background());
                    } else {
                        assert!((32..=126).contains(&(cell.ch as u32)));
                        characters += usize::from(cell.ch != ' ' && cell.attribute.foreground() != cell.attribute.background());
                    }
                    assert!(cell.attribute.foreground() < 16 && cell.attribute.background() < 8);
                    assert!(!cell.attribute.is_blinking());
                }
            }
            if glyphs == Glyphs::HalfBlocks {
                assert!(halves > 0, "must actually draw independent upper/lower colors, not just solids");
            } else {
                assert!(characters > 0, "must draw visible ASCII characters, not just background colors");
            }
        }
    }

    #[test]
    fn image_import_standard_sizes_produce_complete_previews() {
        let source = source();
        for (rows, preset) in [(25, Preset::Scene), (50, Preset::Shaded)] {
            let mut options = Options { rows, ..Default::default() };
            options.select_preset(preset);
            let result = convert(&source, &options).unwrap();
            assert_eq!(result.buffer.size(), Size::new(80, rows));
            assert_eq!(result.preview.size, [640, rows as usize * 16]);
            assert_eq!(result.preview.pixels.len(), 640 * rows as usize * 16);
        }
    }

    #[test]
    fn image_import_focus_drag_and_overlay_follow_the_image_not_the_column() {
        use_english();
        let context = egui::Context::default();
        let mut dialog = ImportDialog {
            source: Some(source()),
            ..Default::default()
        };
        let render = |dialog: &mut ImportDialog, events| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        ui.columns(2, |columns| dialog.source_preview(&mut columns[0], 450.0));
                    });
                },
            )
        };
        let output = render(&mut dialog, vec![]);
        let texture = dialog.source_texture.as_ref().unwrap().id();
        let image = context
            .tessellate(output.shapes.clone(), output.pixels_per_point)
            .iter()
            .find_map(|primitive| match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
                _ => None,
            })
            .unwrap();
        let overlay = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.stroke.color == appearance::PRIMARY => Some(rect.rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(overlay.size(), egui::vec2(360.0, 450.0));
        // Tessellation adds a subpixel antialiasing fringe around the image.
        assert!((overlay.min - image.min).length() <= 1.0 && (overlay.max - image.max).length() <= 1.0);
        let start = overlay.min + overlay.size() * 0.25;
        let end = overlay.min + overlay.size() * 0.75;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(&mut dialog, vec![egui::Event::PointerMoved(start)]);
        render(&mut dialog, vec![button(start, true)]);
        render(&mut dialog, vec![egui::Event::PointerMoved(end)]);
        render(&mut dialog, vec![button(end, false)]);
        assert_eq!(dialog.options.focus, egui::Rect::from_min_max(egui::pos2(0.25, 0.25), egui::pos2(0.75, 0.75)));
    }

    #[test]
    fn image_import_welcome_new_and_file_menu_open_the_dialog() {
        use_english();
        for entry in ["welcome", "new", "file"] {
            let context = egui::Context::default();
            appearance::apply(&context);
            let mut app = DrawApp::new();
            let size = egui::vec2(1280.0, 900.0);
            match entry {
                "welcome" => app.show_start = true,
                "new" => app.request_new(),
                _ => {}
            }
            frame(&context, &mut app, size, vec![]);
            if entry == "file" {
                click_text(&context, &mut app, size, "File");
            }
            click_text(&context, &mut app, size, &fl!("menu-ai-import"));
            if entry == "new" {
                click_text(&context, &mut app, size, &fl!("new-file-create"));
            }
            assert!(matches!(app.dialog, Some(Dialog::AiImport(_))), "{entry}");
            assert!(!app.document.modified());
            for _ in 0..3 {
                frame(&context, &mut app, size, vec![]);
            }
            click_text(&context, &mut app, size, &labels::cancel());
            assert!(app.dialog.is_none(), "{entry}: cancel must close import");
            assert_eq!(app.show_start, entry == "welcome");
        }
    }

    #[test]
    fn image_import_picker_conversion_and_accept_switch_to_ansi() {
        use_english();
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        app.open_ai_import();
        let size = egui::vec2(1280.0, 900.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        source().pixels().unwrap().save(&path).unwrap();
        app.picker = true;
        app.sender
            .send(Picked {
                action: FileAction::AiImport,
                path: Some(path),
            })
            .unwrap();
        wait(&mut app, &context, size);
        assert!(!app.picker);
        let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
        assert!(dialog.source.is_some());
        dialog.options.columns = 8;
        dialog.options.rows = 5;
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, &fl!("ai-import-convert"));
        wait(&mut app, &context, size);
        let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
        assert!(dialog.error.is_none(), "{:?}", dialog.error);
        let expected = dialog.result.as_ref().unwrap().preview.clone();
        assert!(app.font_editor.is_some(), "conversion must not replace the live editor early");
        click_text(&context, &mut app, size, &fl!("ai-import-accept"));
        assert!(app.dialog.is_none() && app.font_editor.is_none());
        assert!(app.rip.is_none() && app.igs.is_none() && app.charfont.is_none() && app.animation.is_none());
        assert!(!app.show_start);
        assert!(app.document.path.is_none() && app.document.modified());
        app.document.with_state(|state| {
            let buffer = state.get_buffer();
            assert_eq!(buffer.size(), Size::new(8, 5));
            let (_, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, 8, 5).into(), false);
            assert_eq!(egui::ColorImage::from_rgba_unmultiplied([64, 80], &pixels), expected);
        });
        app.document.type_text("X").unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'X');
    }

    #[test]
    fn image_import_setting_changes_and_errors_cannot_accept_stale_results() {
        use_english();
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
        let size = egui::vec2(1280.0, 900.0);
        frame(&context, &mut app, size, vec![]);
        click_text(&context, &mut app, size, &fl!("ai-import-shaded"));
        let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
        assert!(dialog.result.is_none());
        dialog.load(&context, Path::new("/nonexistent/icy-import-test.png"));
        wait(&mut app, &context, size);
        let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
        assert!(dialog.error.is_some() && dialog.result.is_none() && dialog.source.is_none());
        assert!(!app.document.modified());
    }

    #[test]
    fn image_import_accept_protects_unsaved_work_on_cancel_discard_and_save() {
        use_english();
        for action in ["cancel", "discard", "save", "picker-cancel", "save-failed"] {
            let context = egui::Context::default();
            let mut app = DrawApp::new();
            app.document.type_text("KEEP").unwrap();
            let result = ready_dialog().result.unwrap().buffer.clone();
            app.import_ansi(result);
            assert!(matches!(app.dialog, Some(Dialog::Close)));
            assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
            let size = egui::vec2(1280.0, 900.0);
            match action {
                "cancel" => click_text(&context, &mut app, size, &labels::cancel()),
                "discard" => click_text(&context, &mut app, size, &fl!("ask_close_file_dialog-dont_save_button")),
                "picker-cancel" => {
                    app.continue_after_save = true;
                    app.picker = true;
                    app.sender
                        .send(Picked {
                            action: FileAction::Save,
                            path: None,
                        })
                        .unwrap();
                    frame(&context, &mut app, size, vec![]);
                }
                _ => {
                    let directory = tempfile::tempdir().unwrap();
                    let path = if action == "save" {
                        directory.path().join("old.icy")
                    } else {
                        directory.path().join("missing/old.icy")
                    };
                    app.continue_after_save = true;
                    app.save_path(&context, path.clone(), true);
                    if action == "save" {
                        let saved = icy_draw::document::Document::load(&path).unwrap();
                        assert_eq!(saved.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
                    }
                }
            }
            assert!(app.pending_import.is_none(), "{action}");
            let imported = matches!(action, "discard" | "save");
            assert_eq!(
                app.document.with_state(|state| state.get_buffer().size()) == Size::new(8, 5),
                imported,
                "{action}"
            );
            if !imported {
                assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
            }
        }
    }

    #[test]
    #[ignore = "requires a graphics adapter"]
    fn gpu_image_import_dialog_previews_at_wide_and_narrow_sizes() {
        use_english();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut gpu = Gpu::new().await;
            let mut app = DrawApp::new();
            for size in [[1280, 900], [440, 700]] {
                app.dialog = Some(Dialog::AiImport(Box::default()));
                for _ in 0..3 {
                    gpu.capture(&mut app, size, 1.0, vec![], "ai-import-warmup");
                }
                let empty = gpu.capture(&mut app, size, 1.0, vec![], &format!("ai-import-empty-{}", size[0]));
                app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
                for _ in 0..3 {
                    gpu.capture(&mut app, size, 1.0, vec![], "ai-import-warmup");
                }
                let rendered = gpu.capture(&mut app, size, 1.0, vec![], &format!("ai-import-preview-{}", size[0]));
                assert!(empty.chunks_exact(4).zip(rendered.chunks_exact(4)).filter(|(a, b)| a != b).count() > 500);
            }
        });
    }
}
