use eframe::egui;
use egui_file::FileDialog;
use std::path::PathBuf;

use crate::{MainWindow, Message};

pub struct SaveFileDialog {
    open_file: bool,
    dialog: FileDialog,
    opened_file: Option<PathBuf>,
}

impl SaveFileDialog {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut dialog = FileDialog::save_file(initial_path);
        dialog.open();

        Self {
            open_file: false,
            dialog,
            opened_file: None,
        }
    }
}

impl crate::ModalDialog for SaveFileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;

        if self.dialog.show(ctx).selected() {
            if let Some(file) = self.dialog.path() {
                self.opened_file = Some(file.to_path_buf());
                self.open_file = true;
            }
            result = true;
        }

        result
    }

    fn should_commit(&self) -> bool {
        self.open_file
    }

    fn commit_self(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        if let Some(file) = &self.opened_file.clone() {
            if let Some(pane) = window.get_active_pane_mut() {
                pane.set_path(file.clone());
                pane.save();
                window.current_id = None;
            }
        }
        Ok(None)
    }
}
