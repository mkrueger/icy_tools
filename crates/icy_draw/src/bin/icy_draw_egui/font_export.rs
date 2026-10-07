//! Exports a snapshot of the bitmap font without changing its save path or baseline.

use std::path::{Path, PathBuf};

use eframe::egui;
use icy_draw::{
    fl,
    font_export::{self, ComExportFormat, FontExportFormat},
};
use icy_engine::BitFont;
use icy_engine_gui::egui::appearance::{self, labels, Dialog, DialogButton, DialogSize, MessageBox, MessageKind};

use super::font_select;

pub enum Action {
    Browse,
    Close,
}

#[derive(Clone)]
pub struct FontExportDialog {
    font: BitFont,
    format: FontExportFormat,
    com_format: ComExportFormat,
    path: String,
    preview: Option<egui::TextureHandle>,
    overwrite: Option<PathBuf>,
    error: Option<String>,
}

impl FontExportDialog {
    pub fn new(font: BitFont) -> Self {
        Self {
            font,
            format: FontExportFormat::default(),
            com_format: ComExportFormat::default(),
            path: String::new(),
            preview: None,
            overwrite: None,
            error: None,
        }
    }

    pub fn extension(&self) -> String {
        self.format.extension(self.font.size().height)
    }

    pub fn default_filename(&self) -> String {
        let name = if self.font.name().is_empty() {
            fl!("unsaved-title")
        } else {
            self.font.name().to_string()
        };
        format!("{name}.{}", self.extension())
    }

    pub fn set_path(&mut self, path: &Path) {
        self.path = path.with_extension(self.extension()).display().to_string();
        self.error = None;
    }

    pub(super) fn set_format(&mut self, format: FontExportFormat) {
        self.format = format;
        if !self.path.trim().is_empty() {
            self.set_path(&PathBuf::from(&self.path));
        }
        self.error = None;
    }

    fn write(&mut self, path: &Path, overwrite: bool) -> bool {
        let result = font_export::encode(&self.font, self.format, self.com_format).and_then(|bytes| icy_draw::files::save_bytes(path, &bytes, None, overwrite));
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn export(&mut self) -> bool {
        self.error = None;
        if self.path.trim().is_empty() {
            self.error = Some(fl!("font-export-no-path"));
            return false;
        }
        let path = PathBuf::from(&self.path).with_extension(self.extension());
        self.set_path(&path);
        if path.exists() {
            self.overwrite = Some(path);
            false
        } else {
            self.write(&path, false)
        }
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        if let Some(path) = self.overwrite.clone() {
            return self.confirm_overwrite(context, &path, blocked);
        }
        #[derive(Clone, Copy)]
        enum Button {
            Export,
            Cancel,
        }
        let mut action = None;
        let response = Dialog::new("font-export").size(DialogSize::Width(720.0)).show(context, |dialog| {
            dialog.content(|ui| {
                if blocked {
                    ui.disable();
                }
                ui.heading(fl!("menu-export-font").trim_end_matches('…'));
                ui.add_space(12.0);
                egui::Grid::new("font-export-options").spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(fl!("font-export-format"));
                    let before = self.format;
                    egui::ComboBox::from_id_salt("font-export-format")
                        .selected_text(self.format.display_name())
                        .width(240.0)
                        .height(280.0)
                        .show_ui(ui, |ui| {
                            for format in FontExportFormat::ALL {
                                ui.selectable_value(&mut self.format, format, format.display_name());
                            }
                        });
                    if self.format != before {
                        self.set_format(self.format);
                    }
                    ui.end_row();
                    if self.format == FontExportFormat::Com {
                        ui.label(fl!("font-export-com-format"));
                        egui::ComboBox::from_id_salt("font-export-com-format")
                            .selected_text(self.com_format.display_name())
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for format in ComExportFormat::ALL {
                                    if ui.selectable_value(&mut self.com_format, format, format.display_name()).changed() {
                                        self.error = None;
                                    }
                                }
                            });
                        ui.end_row();
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(fl!("font-export-path"));
                    let width = (ui.available_width() - 88.0 - ui.spacing().item_spacing.x).max(40.0);
                    if ui
                        .add_sized([width, 28.0], appearance::text_edit(&mut self.path).hint_text(fl!("font-export-no-path")))
                        .changed()
                    {
                        self.error = None;
                    }
                    if ui.add_sized([88.0, 28.0], egui::Button::new(fl!("font-import-browse"))).clicked() {
                        action = Some(Action::Browse);
                    }
                });
                ui.add_space(12.0);
                let body_height = (context.content_rect().height() - 320.0).clamp(100.0, 280.0);
                ui.horizontal_top(|ui| {
                    let preview_width = ui.available_width() * 0.6;
                    ui.allocate_ui_with_layout(egui::vec2(preview_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_min_size(egui::vec2(preview_width, body_height));
                        self.preview(ui);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(self.font.name());
                        ui.weak(format!("{} x {}", self.font.size().width, self.font.size().height));
                        if self.format == FontExportFormat::AnsiDcs {
                            ui.add_space(8.0);
                            ui.label(fl!("font-export-ansi-file-info"));
                        }
                    });
                });
                if let Some(error) = &self.error {
                    ui.colored_label(icy_engine_gui::egui::dialog::DANGER, error);
                }
            });
            dialog.buttons([
                DialogButton::cancel(labels::cancel(), Button::Cancel).enabled(!blocked),
                DialogButton::primary(fl!("font-export-button"), Button::Export).enabled(!blocked && !self.path.trim().is_empty()),
            ]);
        });
        if blocked {
            return None;
        }
        match response.action {
            Some(Button::Export) => self.export().then_some(Action::Close),
            Some(Button::Cancel) => Some(Action::Close),
            _ if response.dismissed => Some(Action::Close),
            _ => action,
        }
    }

    fn confirm_overwrite(&mut self, context: &egui::Context, path: &Path, blocked: bool) -> Option<Action> {
        #[derive(Clone, Copy)]
        enum Button {
            Overwrite,
            Cancel,
        }
        let response = MessageBox::new(
            "font-export-overwrite",
            MessageKind::Warning,
            fl!("replace-export-file"),
            path.display().to_string(),
        )
        .buttons([
            DialogButton::cancel(labels::cancel(), Button::Cancel).enabled(!blocked),
            DialogButton::destructive(labels::overwrite(), Button::Overwrite).enabled(!blocked),
        ])
        .show(context);
        if blocked {
            return None;
        }
        match response.action {
            Some(Button::Overwrite) => {
                self.overwrite = None;
                self.write(path, true).then_some(Action::Close)
            }
            Some(Button::Cancel) => {
                self.overwrite = None;
                None
            }
            _ if response.dismissed => {
                self.overwrite = None;
                None
            }
            _ => None,
        }
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        if self.preview.is_none() {
            match font_select::glyph_sheet(&self.font) {
                Some(image) => self.preview = Some(ui.ctx().load_texture("font-export-preview", image, egui::TextureOptions::NEAREST)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_switches_update_paths_and_exports_use_the_selected_extension() {
        let directory = tempfile::tempdir().unwrap();
        let font = BitFont::default();
        let mut dialog = FontExportDialog::new(font.clone());
        assert_eq!(dialog.extension(), "png");
        assert!(dialog.default_filename().ends_with(".png"));
        dialog.set_path(&directory.path().join("export"));
        assert!(dialog.path.ends_with("export.png"));
        dialog.set_format(FontExportFormat::Raw);
        assert!(dialog.path.ends_with("export.f16"));
        assert!(dialog.export());
        assert_eq!(std::fs::read(&dialog.path).unwrap(), font.convert_to_u8_data());
        dialog.set_format(FontExportFormat::Psf);
        assert!(dialog.path.ends_with("export.psf"));
        assert!(dialog.export());
        let loaded = BitFont::from_bytes("PSF", &std::fs::read(&dialog.path).unwrap()).unwrap();
        assert_eq!(loaded.convert_to_u8_data(), font.convert_to_u8_data());
    }

    #[test]
    fn exporting_preserves_existing_files_until_overwrite_is_confirmed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("export.png");
        std::fs::write(&path, b"KEEP").unwrap();
        let mut dialog = FontExportDialog::new(BitFont::default());
        dialog.set_path(&path);
        assert!(!dialog.export());
        assert_eq!(dialog.overwrite.as_deref(), Some(path.as_path()));
        assert_eq!(std::fs::read(&path).unwrap(), b"KEEP");
        assert!(!dialog.write(&path, false));
        assert!(dialog.error.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), b"KEEP");
        assert!(dialog.write(&path, true));
        assert_eq!(image::open(&path).unwrap().width(), 128);
    }

    #[test]
    fn missing_paths_and_write_failures_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let mut dialog = FontExportDialog::new(BitFont::default());
        assert!(!dialog.export());
        assert!(dialog.error.is_some());
        dialog.set_path(&directory.path().join("missing/export.png"));
        assert!(!dialog.export());
        assert!(dialog.error.is_some());
        assert!(dialog.overwrite.is_none());
        assert!(!Path::new(&dialog.path).exists());
    }
}
