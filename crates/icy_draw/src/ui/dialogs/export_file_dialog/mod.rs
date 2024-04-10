#![allow(clippy::needless_range_loop)]

use std::path::PathBuf;

use eframe::egui::{self, TextEdit, Ui};
use egui_file::FileDialog;
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{BufferType, Rectangle, SaveOptions, TextPane};

use crate::{AnsiEditor, Message, ModalDialog, TerminalResult, SETTINGS};

mod ansi;
mod artworx;
mod ascii;
mod atascii;
mod avatar;
mod bin;
mod ice_draw;
mod pcboard;
mod png;
mod tundra_draw;
mod xbin;

pub struct ExportFileDialog {
    pub should_commit: bool,
    pub file_name: PathBuf,
    folder_dialog: Option<FileDialog>,
    format_type: i32,
    buffer_type: BufferType,
}

impl ExportFileDialog {
    pub fn new(buf: &icy_engine::Buffer) -> Self {
        let file_name = match &buf.file_name {
            Some(path) => {
                let mut p = path.clone();
                let desc: &[(&str, CreateSettingsFunction, &str)] = if matches!(buf.buffer_type, BufferType::Atascii) {
                    &ATASCII_TYPE_DESCRIPTIONS
                } else {
                    &TYPE_DESCRIPTIONS
                };
                let format_type = get_format_type(buf.buffer_type, path) as usize;
                let ext = desc[format_type].2;
                p.set_extension(ext);
                p
            }
            _ => PathBuf::from("Untitled.ans"),
        };
        let format_type = get_format_type(buf.buffer_type, &file_name);

        ExportFileDialog {
            should_commit: false,
            file_name,
            format_type,
            folder_dialog: None,
            buffer_type: buf.buffer_type,
        } // self.file_name.set_extension(TYPE_DESCRIPTIONS[format_type].2);
    }
}

fn get_format_type(buf: BufferType, path: &std::path::Path) -> i32 {
    if let Some(ext) = path.extension() {
        if let Some(ext) = ext.to_str() {
            let ext = ext.to_lowercase();
            let desc: &[(&str, CreateSettingsFunction, &str)] = if matches!(buf, BufferType::Atascii) {
                &ATASCII_TYPE_DESCRIPTIONS
            } else {
                &TYPE_DESCRIPTIONS
            };
            for i in 0..desc.len() {
                let td = desc[i];
                if ext == td.2 {
                    return i as i32;
                }
            }
        }
    }
    0
}

impl ModalDialog for ExportFileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        if let Some(ed) = &mut self.folder_dialog {
            if ed.show(ctx).selected() {
                if let Some(res) = ed.path() {
                    self.file_name = res.to_path_buf();
                }
                self.folder_dialog = None
            } else {
                return false;
            }
        }

        let mut result = false;

        let modal = Modal::new(ctx, "export_file-dialog");
        modal.show(|ui| {
            ui.set_width(550.);
            ui.set_height(320.);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "export-title"));

            modal.frame(ui, |ui| {
                let desc: &[(&str, CreateSettingsFunction, &str)] = if matches!(self.buffer_type, BufferType::Atascii) {
                    &ATASCII_TYPE_DESCRIPTIONS
                } else {
                    &TYPE_DESCRIPTIONS
                };

                egui::Grid::new("paste_mode_grid")
                    .num_columns(2)
                    .spacing([4.0, 8.0])
                    .min_row_height(24.)
                    .show(ui, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "export-file-label"));
                        });
                        ui.horizontal(|ui| {
                            let mut path_edit = self.file_name.to_string_lossy().to_string();
                            let response = ui.add(TextEdit::singleline(&mut path_edit).desired_width(450.));
                            if response.changed() {
                                self.file_name = path_edit.into();
                                let format_type = get_format_type(self.buffer_type, &self.file_name);
                                if format_type >= 0 {
                                    self.format_type = format_type;
                                }
                            }
                            if ui.add(egui::Button::new("…").wrap(false)).clicked() {
                                let mut initial_path = None;
                                crate::set_default_initial_directory_opt(&mut initial_path);
                                let mut dialog = FileDialog::save_file(initial_path);
                                dialog.open();
                                self.folder_dialog = Some(dialog);

                                ui.close_menu();
                            }
                        });
                        ui.end_row();

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "export-format-label"));
                        });
                        egui::ComboBox::from_id_source("format_combo")
                            .selected_text(desc[self.format_type as usize].0)
                            .width(190.)
                            .show_ui(ui, |ui| {
                                (0..desc.len()).for_each(|i| {
                                    let td = desc[i];
                                    if ui.selectable_value(&mut self.format_type, i as i32, td.0).clicked() {
                                        self.file_name.set_extension(td.2);
                                    }
                                });
                            });
                        ui.end_row();
                    });

                ui.separator();

                unsafe {
                    desc[self.format_type as usize].1(ui, &mut SETTINGS.save_options);
                }
            });

            ui.add_space(ui.available_height() - 23.0);

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "export-button-title")).clicked() {
                    self.should_commit = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.should_commit
    }

    fn commit(&self, editor: &mut AnsiEditor) -> TerminalResult<Option<crate::Message>> {
        if let Some(ext) = self.file_name.extension() {
            if let Some(ext) = ext.to_str() {
                let ext = ext.to_lowercase();
                if ext == "png" {
                    let lock = &editor.buffer_view.lock();
                    let buf = lock.get_buffer();
                    let (size, pixels) = buf.render_to_rgba(Rectangle::from(0, 0, buf.get_width(), buf.get_height()));
                    let image_buffer = image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels);
                    match image_buffer {
                        Some(img) => {
                            if let Err(err) = img.save(&self.file_name) {
                                return Ok(Some(Message::ShowError(format!("Failed to save image: {}", err))));
                            }
                        }
                        None => {
                            return Ok(Some(Message::ShowError("Failed to save image".to_string())));
                        }
                    }

                    return Ok(None);
                }
            }
        }
        unsafe {
            editor.save_content(self.file_name.as_path(), &SETTINGS.save_options)?;
        }
        Ok(None)
    }
}

type CreateSettingsFunction = fn(&mut Ui, &mut SaveOptions);

const TYPE_DESCRIPTIONS: [(&str, CreateSettingsFunction, &str); 12] = [
    ("Ansi (.ans)", ansi::create_settings_page, "ans"),
    ("Avatar (.avt)", avatar::create_settings_page, "avt"),
    ("PCBoard (.pcb)", pcboard::create_settings_page, "pcb"),
    ("Ascii (.asc)", ascii::create_settings_page, "asc"),
    ("Artworx (.adf)", artworx::create_settings_page, "adf"),
    ("Ice Draw (.idf)", ice_draw::create_settings_page, "idf"),
    ("Tundra Draw (.tnd)", tundra_draw::create_settings_page, "tnd"),
    ("Bin (.bin)", bin::create_settings_page, "bin"),
    ("XBin (.xb)", xbin::create_settings_page, "xb"),
    ("CtrlA (.msg)", pcboard::create_settings_page, "msg"),
    ("Renegade (.an1)", pcboard::create_settings_page, "an1"),
    ("PNG (.png)", png::create_settings_page, "png"),
];

const ATASCII_TYPE_DESCRIPTIONS: [(&str, CreateSettingsFunction, &str); 3] = [
    ("Atascii (.ata)", atascii::create_settings_page, "ata"),
    ("XBin (.xb)", xbin::create_settings_page, "xb"),
    ("PNG (.png)", png::create_settings_page, "png"),
];
