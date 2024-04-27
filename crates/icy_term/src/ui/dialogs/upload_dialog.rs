use eframe::egui::{self};
use egui_file::FileDialog;
use icy_net::protocol::TransferProtocolType;

use crate::ui::{MainWindow, MainWindowMode};

#[derive(Default)]
pub struct DialogState {
    open_file_dialog: Option<FileDialog>,
    protocol_type: TransferProtocolType,
}

impl MainWindow {
    pub fn init_upload_dialog(&mut self, protocol_type: TransferProtocolType) {
        let mut dialog: FileDialog = FileDialog::open_file(self.initial_upload_directory.clone()).multi_select(true);
        dialog.open();
        self.upload_dialog.open_file_dialog = Some(dialog);
        self.upload_dialog.protocol_type = protocol_type;
        self.set_mode(MainWindowMode::ShowUploadDialog);
    }

    pub fn show_upload_dialog(&mut self, ctx: &egui::Context) {
        if ctx.input(|i: &egui::InputState| i.key_down(egui::Key::Escape)) {
            self.set_mode(MainWindowMode::ShowTerminal);
        }

        if let Some(dialog) = &mut self.upload_dialog.open_file_dialog {
            if dialog.show(ctx).selected() {
                let files = dialog.selection();
                if !files.is_empty() {
                    if matches!(self.upload_dialog.protocol_type, TransferProtocolType::ASCII) {
                        for path in files {
                            match std::fs::read(path) {
                                Ok(bytes) => {
                                    let _ = self.tx.send(crate::ui::connect::SendData::Data(bytes));
                                }
                                r => {
                                    log::error!("Error reading file: {:?}", r); 
                                }
                            }
                        }
                        self.set_mode(MainWindowMode::ShowTerminal);
                        return;
                    }
                    if let Some(parent) = files[0].parent() {
                        self.initial_upload_directory = Some(parent.to_path_buf());
                    }
                    let files = files.iter().map(|p| p.to_path_buf()).collect();
                    self.upload(self.upload_dialog.protocol_type.clone(), files);
                }
            }
        }
    }
}
