use std::{ffi::OsStr, fs::File, io::Write};

use eframe::egui::{self};
use egui_file::FileDialog;
use icy_engine::SaveOptions;

use crate::ui::{MainWindow, MainWindowMode};

#[derive(Default)]
pub struct DialogState {
    open_file_dialog: Option<FileDialog>,
}

impl MainWindow {
    pub fn init_export_dialog(&mut self, ctx: &egui::Context) {
        let mut dialog: FileDialog = FileDialog::save_file(None);
        dialog.open();
        self.export_dialog.open_file_dialog = Some(dialog);
        self.set_mode(ctx, MainWindowMode::ShowExportDialog);
    }
    pub fn show_export_dialog(&mut self, ctx: &egui::Context) {
        if ctx.input(|i: &egui::InputState| i.key_down(egui::Key::Escape)) {
            self.set_mode(ctx, MainWindowMode::ShowTerminal);
        }

        if let Some(dialog) = &mut self.export_dialog.open_file_dialog {
            if dialog.show(ctx).selected() {
                if let Some(path) = dialog.path() {
                    if let Some(file_name) = path.to_str() {
                        if let Ok(mut file) = File::create(file_name) {
                            let content = if let Some(ext) = path.extension() {
                                let ext = OsStr::to_str(ext).unwrap().to_lowercase();
                                self.buffer_view.lock().get_buffer().to_bytes(ext.as_str(), &SaveOptions::new())
                            } else {
                                self.buffer_view.lock().get_buffer().to_bytes("ans", &SaveOptions::new())
                            };
                            let r = match content {
                                Ok(content) => file.write_all(&content),
                                Err(err) => file.write_all(err.to_string().as_bytes()),
                            };
                            if let Err(err) = r {
                                log::error!("Error writing file: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }
}
