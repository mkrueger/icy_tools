use eframe::egui;
use egui_file::FileDialog;
use std::path::PathBuf;

use crate::{AnsiEditor, Message, TerminalResult};

pub struct OpenReferenceImageDialog {
    open_file: bool,
    dialog: FileDialog,
    opened_file: Option<PathBuf>,
}

impl OpenReferenceImageDialog {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        let mut dialog = FileDialog::open_file(initial_path);
        dialog.open();
        Self {
            open_file: false,
            dialog,
            opened_file: None,
        }
    }
}

impl crate::ModalDialog for OpenReferenceImageDialog {
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

    fn commit(&self, editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        if let Some(file) = &self.opened_file.clone() {
            let mut lock = editor.buffer_view.lock();
            lock.load_reference_image(file);
        }
        Ok(None)
    }
}
