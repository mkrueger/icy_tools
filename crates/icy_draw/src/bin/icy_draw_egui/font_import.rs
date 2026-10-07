//! Imports a bitmap font with a glyph-sheet preview before opening it for editing.

use std::path::{Path, PathBuf};

use eframe::egui;
use icy_draw::{
    fl,
    font_import::{
        image_import, is_image_extension, is_native_font_extension, is_ttf_extension, parse_com_font, ttf_import, IMAGE_EXTENSIONS, NATIVE_EXTENSIONS,
        TTF_EXTENSIONS,
    },
};
use icy_engine::BitFont;
use icy_engine_edit::bitfont::{MAX_FONT_HEIGHT, MAX_FONT_WIDTH};
use icy_engine_gui::egui::appearance::{self, labels, Dialog, DialogButton, DialogSize};

use super::font_select;

pub enum Action {
    Browse,
    Import(Box<BitFont>),
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Native,
    XBin,
    Image,
    TrueType,
}

#[derive(Clone)]
pub struct FontImportDialog {
    path: String,
    source: Option<Source>,
    fonts: Vec<BitFont>,
    selected: usize,
    width: String,
    height: String,
    dithering: bool,
    preview: Option<egui::TextureHandle>,
    error: Option<String>,
}

impl Default for FontImportDialog {
    fn default() -> Self {
        Self {
            path: String::new(),
            source: None,
            fonts: Vec::new(),
            selected: 0,
            width: "8".into(),
            height: "16".into(),
            dithering: true,
            preview: None,
            error: None,
        }
    }
}

impl FontImportDialog {
    pub fn load(&mut self, path: &Path) {
        self.path = path.to_string_lossy().into_owned();
        self.fonts.clear();
        self.selected = 0;
        self.preview = None;
        self.error = None;
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        self.source = if is_native_font_extension(&extension) || extension == "com" {
            Some(Source::Native)
        } else if extension == "xb" || extension == "xbin" {
            Some(Source::XBin)
        } else if is_ttf_extension(&extension) {
            Some(Source::TrueType)
        } else if is_image_extension(&extension) {
            Some(Source::Image)
        } else {
            self.error = Some(format!("Unsupported file type: .{extension}"));
            None
        };
        match self.source {
            Some(Source::Image) => match image::image_dimensions(path) {
                Ok((width, height)) => {
                    self.width = (width / 16).clamp(1, MAX_FONT_WIDTH as u32).to_string();
                    self.height = (height / 16).clamp(1, MAX_FONT_HEIGHT as u32).to_string();
                    self.convert();
                }
                Err(error) => self.error = Some(format!("Failed to load image: {error}")),
            },
            Some(Source::TrueType) => self.convert(),
            Some(Source::Native) if extension == "com" => {
                let result = std::fs::read(path)
                    .map_err(|error| format!("Failed to read file: {error}"))
                    .and_then(|data| parse_com_font(path.file_stem().and_then(|name| name.to_str()).unwrap_or("Font"), &data))
                    .map(|font| vec![font]);
                self.set_fonts(result);
            }
            Some(Source::Native | Source::XBin) => self.set_fonts(font_select::load_fonts(path)),
            None => {}
        }
    }

    fn set_fonts(&mut self, result: Result<Vec<BitFont>, String>) {
        self.fonts.clear();
        self.preview = None;
        self.error = None;
        match result {
            Ok(fonts) => {
                if fonts.iter().any(|font| !(1..=MAX_FONT_WIDTH).contains(&font.size().width)) {
                    self.error = Some(fl!("error-bitmap-font-width"));
                } else if fonts.iter().any(|font| !(1..=MAX_FONT_HEIGHT).contains(&font.size().height)) {
                    self.error = Some(fl!("font-import-invalid-height", max = MAX_FONT_HEIGHT));
                } else if fonts.is_empty() {
                    self.error = Some(fl!("set-font-xbin-no-fonts"));
                } else {
                    self.fonts = fonts;
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn dimensions(&self) -> Result<(i32, i32), String> {
        let minimum = if self.source == Some(Source::TrueType) { 4 } else { 1 };
        let width = self.width.parse::<i32>().ok().filter(|width| (minimum..=MAX_FONT_WIDTH).contains(width));
        let height = self.height.parse::<i32>().ok().filter(|height| (minimum..=MAX_FONT_HEIGHT).contains(height));
        width
            .zip(height)
            .ok_or_else(|| fl!("font-import-invalid-size", min = minimum, width = MAX_FONT_WIDTH, height = MAX_FONT_HEIGHT))
    }

    fn convert(&mut self) {
        let result = self.dimensions().and_then(|(width, height)| {
            let path = PathBuf::from(&self.path);
            if self.source == Some(Source::TrueType) {
                ttf_import::import_font_from_ttf(&path, width, height)
            } else {
                image_import::import_font_from_image(&path, width, height, self.dithering)
            }
        });
        self.set_fonts(result.map(|font| vec![font]));
    }

    fn can_import(&self) -> bool {
        self.error.is_none() && self.fonts.get(self.selected).is_some()
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        #[derive(Clone, Copy)]
        enum Button {
            Import,
            Cancel,
        }
        let mut action = None;
        let title = fl!("menu-import-font").trim_end_matches('…').to_string();
        let response = Dialog::new("font-import").size(DialogSize::Width(800.0)).show(context, |dialog| {
            dialog.content(|ui| {
                if blocked {
                    ui.disable();
                }
                ui.heading(&title);
                ui.add_space(12.0);
                let mut load = false;
                ui.horizontal(|ui| {
                    ui.label(fl!("font-import-file"));
                    let browse_width = 88.0;
                    let width = (ui.available_width() - browse_width - ui.spacing().item_spacing.x).max(40.0);
                    load = ui
                        .add_sized(
                            [width, 28.0],
                            appearance::text_edit(&mut self.path).hint_text(fl!("font-import-file-placeholder")),
                        )
                        .changed();
                    if ui.add_sized([browse_width, 28.0], egui::Button::new(fl!("font-import-browse"))).clicked() {
                        action = Some(Action::Browse);
                    }
                });
                if load {
                    if self.path.trim().is_empty() {
                        self.source = None;
                        self.fonts.clear();
                        self.preview = None;
                        self.error = None;
                    } else {
                        self.load(&PathBuf::from(&self.path));
                    }
                }
                ui.add_space(12.0);
                let body_height = (context.content_rect().height() - 260.0).clamp(120.0, 340.0);
                ui.horizontal_top(|ui| {
                    let preview_width = ui.available_width() * 0.6;
                    ui.allocate_ui_with_layout(egui::vec2(preview_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_min_size(egui::vec2(preview_width, body_height));
                        self.preview(ui);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| self.options(ui));
                });
                if let Some(error) = &self.error {
                    ui.colored_label(icy_engine_gui::egui::dialog::DANGER, error);
                }
            });
            dialog.buttons([
                DialogButton::cancel(labels::cancel(), Button::Cancel).enabled(!blocked),
                DialogButton::primary(fl!("font-import-button"), Button::Import).enabled(!blocked && self.can_import()),
            ]);
        });
        if blocked {
            return None;
        }
        match response.action {
            Some(Button::Import) if self.can_import() => self.fonts.get(self.selected).cloned().map(|font| Action::Import(Box::new(font))),
            Some(Button::Cancel) => Some(Action::Cancel),
            _ if response.dismissed => Some(Action::Cancel),
            _ => action,
        }
    }

    fn options(&mut self, ui: &mut egui::Ui) {
        match self.source {
            Some(Source::Native) => {
                ui.label(fl!("font-import-native-info"));
                if let Some(font) = self.fonts.first() {
                    ui.label(font.name());
                    ui.weak(format!("{} x {}", font.size().width, font.size().height));
                }
            }
            Some(Source::XBin) => {
                ui.label(fl!("font-import-xb-info"));
                ui.label(fl!("font-import-select-font"));
                let before = self.selected;
                for (index, _) in self.fonts.iter().enumerate() {
                    let label = if index == 0 {
                        fl!("font-import-xb-font-1")
                    } else {
                        fl!("font-import-xb-font-2")
                    };
                    ui.radio_value(&mut self.selected, index, label);
                }
                if before != self.selected {
                    self.preview = None;
                }
            }
            Some(Source::Image | Source::TrueType) => {
                ui.label(if self.source == Some(Source::Image) {
                    fl!("font-import-image-info")
                } else {
                    fl!("font-import-ttf-info")
                });
                ui.add_space(8.0);
                let mut changed = false;
                egui::Grid::new("font-import-dimensions").show(ui, |ui| {
                    ui.label(fl!("font-size-width"));
                    changed |= ui.add(appearance::text_edit(&mut self.width).desired_width(60.0)).changed();
                    ui.end_row();
                    ui.label(fl!("font-size-height"));
                    changed |= ui.add(appearance::text_edit(&mut self.height).desired_width(60.0)).changed();
                    ui.end_row();
                });
                if self.source == Some(Source::Image) {
                    changed |= ui.checkbox(&mut self.dithering, fl!("font-import-dithering")).changed();
                }
                if changed {
                    self.convert();
                }
            }
            None => {}
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let Some(font) = self.fonts.get(self.selected) else {
            ui.centered_and_justified(|ui| ui.weak(fl!("font-import-no-preview")));
            return;
        };
        if self.preview.is_none() {
            match font_select::glyph_sheet(font) {
                Some(image) => self.preview = Some(ui.ctx().load_texture("font-import-preview", image, egui::TextureOptions::NEAREST)),
                None => {
                    self.error = Some(fl!("font-import-no-preview"));
                    return;
                }
            }
        }
        if let Some(texture) = &self.preview {
            let size = texture.size_vec2();
            let room = ui.available_size();
            let fit = (room.x / size.x).min(room.y / size.y);
            let scale = if fit >= 1.0 { fit.floor().min(2.0) } else { fit.max(0.1) };
            ui.centered_and_justified(|ui| ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale)));
        }
    }
}

pub fn file_dialog(dialog: rfd::FileDialog) -> rfd::FileDialog {
    let all: Vec<_> = NATIVE_EXTENSIONS
        .iter()
        .chain(TTF_EXTENSIONS)
        .chain(IMAGE_EXTENSIONS)
        .copied()
        .chain(["xb", "xbin", "com"])
        .collect();
    dialog
        .set_title(fl!("menu-import-font").trim_end_matches('…'))
        .add_filter(fl!("menu-import-font"), &all)
        .add_filter(fl!("file-dialog-filter-font-files"), NATIVE_EXTENSIONS)
        .add_filter("TrueType/OpenType", TTF_EXTENSIONS)
        .add_filter("XBin", &["xb", "xbin"])
        .add_filter("DOS COM", &["com"])
        .add_filter(fl!("file-dialog-filter-images"), IMAGE_EXTENSIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_import_and_invalid_path_clear_previous_preview() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.PSF");
        let font = BitFont::default();
        std::fs::write(&path, font.to_psf2_bytes().unwrap()).unwrap();
        let mut dialog = FontImportDialog::default();
        assert!(!dialog.can_import());
        dialog.load(&path);
        assert!(dialog.can_import());
        assert_eq!(dialog.fonts[0].convert_to_u8_data(), font.convert_to_u8_data());
        dialog.load(&directory.path().join("missing.psf"));
        assert!(!dialog.can_import());
        assert!(dialog.error.is_some());
        assert!(dialog.fonts.is_empty());
        dialog.load(&directory.path().join("unsupported.txt"));
        assert_eq!(dialog.source, None);
        assert!(!dialog.can_import());
    }

    #[test]
    fn image_import_detects_dimensions_and_invalid_options_clear_preview() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sheet.png");
        let mut image = image::RgbImage::new(128, 256);
        for (x, _, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgb(if x % 8 == 0 { [255; 3] } else { [0; 3] });
        }
        image.save(&path).unwrap();
        let mut dialog = FontImportDialog::default();
        dialog.load(&path);
        assert!(dialog.can_import(), "{:?}", dialog.error);
        assert_eq!((&dialog.width[..], &dialog.height[..]), ("8", "16"));
        assert_eq!(dialog.fonts[0].convert_to_u8_data(), vec![0x80; 256 * 16]);
        dialog.width = "9".into();
        dialog.convert();
        assert!(!dialog.can_import());
        assert!(dialog.fonts.is_empty());
        assert!(dialog.error.is_some());
        dialog.width = "8".into();
        dialog.height = "33".into();
        dialog.convert();
        assert!(!dialog.can_import());
        dialog.height = "16".into();
        dialog.dithering = false;
        dialog.convert();
        assert!(dialog.can_import());
    }

    #[test]
    fn truetype_dimensions_and_parse_failures_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invalid.ttf");
        std::fs::write(&path, b"not a font").unwrap();
        let mut dialog = FontImportDialog::default();
        dialog.load(&path);
        assert!(!dialog.can_import());
        assert!(dialog.error.as_ref().unwrap().contains("Failed to parse font"));
        dialog.width = "3".into();
        assert!(dialog.dimensions().is_err());
        dialog.width = "8".into();
        dialog.height = "32".into();
        assert_eq!(dialog.dimensions().unwrap(), (8, 32));
    }

    #[test]
    fn truetype_import_rasterizes_256_cp437_glyphs() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../icy_engine_gui/data/fonts/FiraSans-Regular.ttf");
        let mut dialog = FontImportDialog::default();
        dialog.load(&path);
        assert!(dialog.can_import(), "{:?}", dialog.error);
        let font = &dialog.fonts[0];
        assert_eq!(font.size(), icy_engine::Size::new(8, 16));
        assert_eq!(font.length(), 256);
        assert!(font.convert_to_u8_data().iter().any(|byte| *byte != 0));
        dialog.height = "8".into();
        dialog.convert();
        assert!(dialog.can_import());
        assert_eq!(dialog.fonts[0].size(), icy_engine::Size::new(8, 8));
    }

    #[test]
    fn xbin_import_keeps_both_embedded_fonts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("two.xb");
        let mut data = b"XBIN\x1A\x01\x00\x01\x00\x10\x12".to_vec();
        data.extend(vec![0x80; 256 * 16]);
        data.extend(vec![0x40; 256 * 16]);
        data.extend([0, 7]);
        std::fs::write(&path, data).unwrap();
        let mut dialog = FontImportDialog::default();
        dialog.load(&path);
        assert!(dialog.can_import(), "{:?}", dialog.error);
        assert_eq!(dialog.source, Some(Source::XBin));
        assert_eq!(dialog.fonts.len(), 2);
        assert_eq!(dialog.fonts[0].convert_to_u8_data(), vec![0x80; 256 * 16]);
        assert_eq!(dialog.fonts[1].convert_to_u8_data(), vec![0x40; 256 * 16]);
    }
}
