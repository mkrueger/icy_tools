use eframe::egui;
use egui_modal::Modal;
use i18n_embed_fl::fl;
use std::{path::PathBuf, sync::Arc};

use crate::{MainWindow, Message};

pub struct OpenFileDialog {
    open_file: bool,
    dialog: view_library::MainWindow,
    opened_file: Option<PathBuf>,
}

impl OpenFileDialog {
    pub fn new(gl: &Arc<glow::Context>, initial_path: Option<PathBuf>) -> Self {
        let mut dialog = view_library::MainWindow::new(gl, initial_path);
        Self {
            open_file: false,
            dialog,
            opened_file: None,
        }
    }
}

impl crate::ModalDialog for OpenFileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "open_file_dialog");

        modal.show(|ui| {
          //  ui.set_width(800.0);
            ui.set_height(600.0);

            modal.frame(ui, |ui| {
                self.dialog.show_file_chooser(ui);
            });

            modal.buttons(ui, |ui| {
                if ui
                    .button(fl!(crate::LANGUAGE_LOADER, "new-file-ok"))
                    .clicked()
                {
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.open_file
    }

    fn commit_self(&self, _: &mut MainWindow) -> crate::TerminalResult<Option<Message>> {
        if let Some(file) = &self.opened_file.clone() {
            Ok(Some(Message::TryLoadFile(file.clone())))
        } else {
            Ok(None)
        }
    }
}
